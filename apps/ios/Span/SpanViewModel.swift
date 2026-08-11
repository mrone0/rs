import Combine
import Foundation

@MainActor
final class SpanViewModel: ObservableObject {
    @Published var devices: [SpanDevice] = []
    @Published var statusMessage: String = "Ready"
    @Published private(set) var localIdentity: LocalSpanIdentity

    @Published var manualDeviceID: String = ""
    @Published var manualName: String = ""
    @Published var manualHost: String = ""
    @Published var manualPublicKeyHex: String = ""

    private let clipboard: ClipboardService
    private let dispatcher: SpanDispatcher
    private let discovery: DiscoveryService
    private let deviceStore: TrustedDeviceStore

    init(
        clipboard: ClipboardService = SystemClipboardService(),
        transport: SpanTransport? = nil,
        discovery: DiscoveryService? = nil,
        deviceStore: TrustedDeviceStore = UserDefaultsTrustedDeviceStore(),
        identityStore: LocalIdentityStore = UserDefaultsLocalIdentityStore()
    ) {
        let dispatcher = SpanDispatcher(
            clipboard: clipboard,
            transport: transport ?? NetworkSpanTransport(),
            identityStore: identityStore,
            deviceStore: deviceStore
        )
        let identity = identityStore.loadOrCreate()
        self.localIdentity = identity
        self.clipboard = clipboard
        self.dispatcher = dispatcher
        self.discovery = discovery ?? UDPDiscoveryService(identity: identity)
        self.deviceStore = deviceStore
        self.devices = deviceStore.loadDevices()

        self.discovery.onDeviceUpdated = { [weak self] device in
            Task { @MainActor in
                self?.upsert(device)
            }
        }
    }

    var localSummary: String {
        "id: \(localIdentity.id)\nplatform: ios\npublic key: \(localIdentity.publicKeyHex)"
    }

    func start() {
        discovery.start()
        statusMessage = "Discovery running: \(localIdentity.name)"
    }

    func stop() {
        discovery.stop()
        statusMessage = "Stopped"
    }

    func announce() {
        discovery.announce()
        statusMessage = "Announced to local network"
    }

    func sendCurrentClipboard() {
        guard let text = clipboard.currentText(), !text.isEmpty else {
            statusMessage = "Clipboard is empty"
            return
        }
        send(text)
    }

    func sendCurrentSelection() {
        statusMessage = "Use the iOS Share sheet and choose Send with Span."
    }

    func trust(deviceID: String) {
        guard let index = devices.firstIndex(where: { $0.id == deviceID }) else { return }
        devices[index].isTrusted = true
        persist()
        statusMessage = "Trusted \(devices[index].name)"
    }

    func revoke(deviceID: String) {
        guard let index = devices.firstIndex(where: { $0.id == deviceID }) else { return }
        devices[index].isTrusted = false
        persist()
        statusMessage = "Revoked \(devices[index].name)"
    }

    func addManualDevice() {
        let id = manualDeviceID.trimmingCharacters(in: .whitespacesAndNewlines)
        let name = manualName.trimmingCharacters(in: .whitespacesAndNewlines)
        let host = manualHost.trimmingCharacters(in: .whitespacesAndNewlines)
        let key = manualPublicKeyHex.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()

        guard !id.isEmpty, !host.isEmpty, Hex.decode(key)?.count == 32 else {
            statusMessage = "Manual device needs id, host, and 32-byte public key"
            return
        }

        upsert(SpanDevice(
            id: id,
            name: name.isEmpty ? id : name,
            platform: "unknown",
            host: host,
            publicKeyHex: key,
            isTrusted: true,
            lastSeenAt: Date()
        ))

        manualDeviceID = ""
        manualName = ""
        manualHost = ""
        manualPublicKeyHex = ""
        statusMessage = "Manual device trusted"
    }

    private func send(_ text: String) {
        Task {
            do {
                let sent = try await dispatcher.sendText(text)
                await MainActor.run {
                    self.statusMessage = sent == 0 ? "No trusted devices" : "Sent to \(sent) trusted device(s)"
                }
            } catch {
                await MainActor.run {
                    self.statusMessage = "Send failed"
                }
            }
        }
    }

    private func upsert(_ discovered: SpanDevice) {
        if let index = devices.firstIndex(where: { $0.id == discovered.id }) {
            let wasTrusted = devices[index].isTrusted || discovered.isTrusted
            devices[index].name = discovered.name
            devices[index].platform = discovered.platform
            devices[index].host = discovered.host
            devices[index].publicKeyHex = discovered.publicKeyHex
            devices[index].lastSeenAt = discovered.lastSeenAt
            devices[index].isTrusted = wasTrusted
        } else {
            devices.append(discovered)
        }
        persist()
        statusMessage = "Found \(discovered.name)"
    }

    private func persist() {
        deviceStore.saveDevices(devices)
    }
}
