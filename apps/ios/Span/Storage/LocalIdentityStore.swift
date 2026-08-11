import CryptoKit
import Foundation
import UIKit

protocol LocalIdentityStore {
    func loadOrCreate() -> LocalSpanIdentity
}

final class UserDefaultsLocalIdentityStore: LocalIdentityStore {
    private let key = "span.local.identity.v1"
    private let defaults: UserDefaults

    init(defaults: UserDefaults = SpanAppGroup.defaults) {
        self.defaults = defaults
    }

    func loadOrCreate() -> LocalSpanIdentity {
        if let data = defaults.data(forKey: key),
           let identity = try? JSONDecoder().decode(LocalSpanIdentity.self, from: data) {
            return identity
        }

        let privateKey = Curve25519.KeyAgreement.PrivateKey()
        let identity = LocalSpanIdentity(
            id: "\(sanitizedDeviceName())-\(Int(Date().timeIntervalSince1970 * 1000))",
            name: sanitizedDeviceName(),
            privateKeyHex: Hex.encode(privateKey.rawRepresentation),
            publicKeyHex: Hex.encode(privateKey.publicKey.rawRepresentation)
        )
        if let data = try? JSONEncoder().encode(identity) {
            defaults.set(data, forKey: key)
        }
        return identity
    }

    private func sanitizedDeviceName() -> String {
        let name = UIDevice.current.name.trimmingCharacters(in: .whitespacesAndNewlines)
        return name.isEmpty ? "iPhone" : name.replacingOccurrences(of: "\t", with: " ")
    }
}
