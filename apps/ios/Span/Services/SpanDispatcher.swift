import Foundation

actor SpanDispatcher {
    private let clipboard: ClipboardService
    private let transport: SpanTransport
    private let identityStore: LocalIdentityStore
    private let deviceStore: TrustedDeviceStore

    init(
        clipboard: ClipboardService = SystemClipboardService(),
        transport: SpanTransport = NetworkSpanTransport(),
        identityStore: LocalIdentityStore = UserDefaultsLocalIdentityStore(),
        deviceStore: TrustedDeviceStore = UserDefaultsTrustedDeviceStore()
    ) {
        self.clipboard = clipboard
        self.transport = transport
        self.identityStore = identityStore
        self.deviceStore = deviceStore
    }

    func sendClipboard() async throws -> Int {
        let text = await MainActor.run { clipboard.currentText() }
        guard let text, !text.isEmpty else {
            return 0
        }
        return try await send(text)
    }

    func sendText(_ text: String) async throws -> Int {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return 0 }
        return try await send(trimmed)
    }

    func trustedDevices() -> [SpanDevice] {
        deviceStore.loadDevices().filter { $0.isTrusted }
    }

    func upsert(_ device: SpanDevice) {
        var devices = deviceStore.loadDevices()
        if let index = devices.firstIndex(where: { $0.id == device.id }) {
            let wasTrusted = devices[index].isTrusted
            devices[index] = device
            devices[index].isTrusted = wasTrusted
        } else {
            devices.append(device)
        }
        deviceStore.saveDevices(devices)
    }

    func setTrusted(_ deviceID: String, trusted: Bool) {
        var devices = deviceStore.loadDevices()
        guard let index = devices.firstIndex(where: { $0.id == deviceID }) else { return }
        devices[index].isTrusted = trusted
        deviceStore.saveDevices(devices)
    }

    private func send(_ text: String) async throws -> Int {
        let identity = identityStore.loadOrCreate()
        let trusted = deviceStore.loadDevices().filter { $0.isTrusted }
        var sent = 0
        for device in trusted {
            try await transport.sendText(text, from: identity, to: device)
            sent += 1
        }
        return sent
    }
}
