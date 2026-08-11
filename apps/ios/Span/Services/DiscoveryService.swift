import Darwin
import Foundation

protocol DiscoveryService: AnyObject {
    var onDeviceUpdated: ((SpanDevice) -> Void)? { get set }
    func start()
    func stop()
    func announce()
}

final class UDPDiscoveryService: DiscoveryService {
    var onDeviceUpdated: ((SpanDevice) -> Void)?

    private let identity: LocalSpanIdentity
    private let queue = DispatchQueue(label: "span.discovery.udp")
    private var socketFD: Int32 = -1
    private var readSource: DispatchSourceRead?
    private var announceTimer: DispatchSourceTimer?

    init(identity: LocalSpanIdentity) {
        self.identity = identity
    }

    func start() {
        queue.async { [weak self] in
            self?.startLocked()
        }
    }

    func stop() {
        queue.async { [weak self] in
            self?.stopLocked()
        }
    }

    func announce() {
        queue.async { [weak self] in
            self?.sendAnnouncement()
        }
    }

    private func startLocked() {
        guard socketFD == -1 else { return }
        let fd = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP)
        guard fd >= 0 else { return }

        var yes: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &yes, socklen_t(MemoryLayout<Int32>.size))
        setsockopt(fd, SOL_SOCKET, SO_BROADCAST, &yes, socklen_t(MemoryLayout<Int32>.size))

        var addr = sockaddr_in()
        addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = SpanProtocolV1.discoveryPort.bigEndian
        addr.sin_addr = in_addr(s_addr: INADDR_ANY.bigEndian)

        let bindResult = withUnsafePointer(to: &addr) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPointer in
                bind(fd, sockaddrPointer, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        guard bindResult == 0 else {
            close(fd)
            return
        }

        socketFD = fd
        let source = DispatchSource.makeReadSource(fileDescriptor: fd, queue: queue)
        source.setEventHandler { [weak self] in
            self?.receiveAvailablePackets()
        }
        source.setCancelHandler {
            close(fd)
        }
        readSource = source
        source.resume()
        scheduleAnnouncementTimer()
        sendAnnouncement()
    }

    private func stopLocked() {
        announceTimer?.cancel()
        announceTimer = nil
        readSource?.cancel()
        readSource = nil
        socketFD = -1
    }

    private func scheduleAnnouncementTimer() {
        announceTimer?.cancel()
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + 15, repeating: 15)
        timer.setEventHandler { [weak self] in
            self?.sendAnnouncement()
        }
        announceTimer = timer
        timer.resume()
    }

    private func receiveAvailablePackets() {
        guard socketFD >= 0 else { return }
        var buffer = [UInt8](repeating: 0, count: 2048)
        var remote = sockaddr_in()
        var remoteLen = socklen_t(MemoryLayout<sockaddr_in>.size)

        let count = withUnsafeMutablePointer(to: &remote) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPointer in
                recvfrom(socketFD, &buffer, buffer.count, 0, sockaddrPointer, &remoteLen)
            }
        }
        guard count > 0 else { return }
        guard let value = String(data: Data(buffer.prefix(Int(count))), encoding: .utf8) else { return }
        guard let device = parsePacket(value, host: remoteHost(remote)), device.id != identity.id else { return }
        onDeviceUpdated?(device)
    }

    private func sendAnnouncement() {
        let fd = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP)
        guard fd >= 0 else { return }
        defer { close(fd) }

        var yes: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_BROADCAST, &yes, socklen_t(MemoryLayout<Int32>.size))

        let packet = "\(SpanProtocolV1.discoveryMagic)\t\(identity.id)\t\(sanitize(identity.name))\tios\t\(identity.publicKeyHex)"
        guard let data = packet.data(using: .utf8) else { return }

        var addr = sockaddr_in()
        addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = SpanProtocolV1.discoveryPort.bigEndian
        addr.sin_addr = in_addr(s_addr: inet_addr("255.255.255.255"))

        _ = data.withUnsafeBytes { bytes in
            withUnsafePointer(to: &addr) { pointer in
                pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPointer in
                    sendto(fd, bytes.baseAddress, data.count, 0, sockaddrPointer, socklen_t(MemoryLayout<sockaddr_in>.size))
                }
            }
        }
    }

    private func parsePacket(_ value: String, host: String?) -> SpanDevice? {
        let fields = value.trimmingCharacters(in: .whitespacesAndNewlines).split(separator: "\t", omittingEmptySubsequences: false).map(String.init)
        guard fields.count >= 5, fields[0] == SpanProtocolV1.discoveryMagic else { return nil }
        guard Hex.decode(fields[4])?.count == 32 else { return nil }
        return SpanDevice(
            id: fields[1],
            name: fields[2],
            platform: fields[3],
            host: host,
            publicKeyHex: fields[4],
            isTrusted: false,
            lastSeenAt: Date()
        )
    }

    private func remoteHost(_ addr: sockaddr_in) -> String? {
        var address = addr.sin_addr
        var buffer = [CChar](repeating: 0, count: Int(INET_ADDRSTRLEN))
        guard inet_ntop(AF_INET, &address, &buffer, socklen_t(INET_ADDRSTRLEN)) != nil else { return nil }
        return String(cString: buffer)
    }

    private func sanitize(_ value: String) -> String {
        value.replacingOccurrences(of: "\t", with: " ")
            .replacingOccurrences(of: "\n", with: " ")
            .replacingOccurrences(of: "\r", with: " ")
    }
}
