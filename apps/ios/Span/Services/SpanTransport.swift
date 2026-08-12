import Foundation
import Network

protocol SpanTransport: Sendable {
    func sendText(_ text: String, from identity: LocalSpanIdentity, to device: SpanDevice) async throws
}

final class NetworkSpanTransport: SpanTransport, @unchecked Sendable {
    func sendText(_ text: String, from identity: LocalSpanIdentity, to device: SpanDevice) async throws {
        guard text.utf8.count <= SpanProtocolV1.maxTextBytes else {
            throw SpanTransportError.textTooLarge
        }
        guard let host = device.host, !host.isEmpty else {
            throw SpanTransportError.missingHost
        }
        guard let publicKeyHex = device.publicKeyHex else {
            throw SpanTransportError.missingPublicKey
        }

        let encrypted = try SpanCrypto.encryptText(
            text,
            localPrivateKeyHex: identity.privateKeyHex,
            peerPublicKeyHex: publicKeyHex
        )
        let line = "\(SpanProtocolV1.textMagic)\t\(identity.id)\t\(encrypted.nonceHex)\t\(encrypted.ciphertextHex)\n"
        guard let payload = line.data(using: .utf8) else {
            throw SpanTransportError.encodingFailed
        }

        try await send(payload, host: host, port: SpanProtocolV1.textPort)
    }

    private func send(_ data: Data, host: String, port: UInt16) async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            let connection = NWConnection(
                host: NWEndpoint.Host(host),
                port: NWEndpoint.Port(rawValue: port)!,
                using: .tcp
            )
            let completion = SpanSendCompletion(connection: connection, continuation: continuation)

            connection.stateUpdateHandler = { state in
                switch state {
                case .ready:
                    connection.send(content: data, completion: .contentProcessed { error in
                        if let error {
                            completion.resume(.failure(error))
                        } else {
                            completion.resume(.success(()))
                        }
                    })
                case .failed(let error):
                    completion.resume(.failure(error))
                case .cancelled:
                    break
                default:
                    break
                }
            }
            connection.start(queue: .global(qos: .userInitiated))
        }
    }
}

private final class SpanSendCompletion: @unchecked Sendable {
    private let connection: NWConnection
    private let continuation: CheckedContinuation<Void, Error>
    private let lock = NSLock()
    private var didResume = false

    init(connection: NWConnection, continuation: CheckedContinuation<Void, Error>) {
        self.connection = connection
        self.continuation = continuation
    }

    func resume(_ result: Result<Void, Error>) {
        lock.lock()
        defer { lock.unlock() }
        guard !didResume else { return }
        didResume = true
        connection.cancel()
        continuation.resume(with: result)
    }
}

enum SpanTransportError: Error {
    case missingHost
    case missingPublicKey
    case textTooLarge
    case encodingFailed
}
