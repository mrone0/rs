import CryptoKit
import Foundation
import Security

enum SpanCrypto {
    static func encryptText(
        _ text: String,
        localPrivateKeyHex: String,
        peerPublicKeyHex: String
    ) throws -> (nonceHex: String, ciphertextHex: String) {
        guard let privateKeyData = Hex.decode(localPrivateKeyHex), privateKeyData.count == 32 else {
            throw SpanCryptoError.badPrivateKey
        }
        guard let peerPublicKeyData = Hex.decode(peerPublicKeyHex), peerPublicKeyData.count == 32 else {
            throw SpanCryptoError.badPeerPublicKey
        }

        let privateKey = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: privateKeyData)
        let peerPublicKey = try Curve25519.KeyAgreement.PublicKey(rawRepresentation: peerPublicKeyData)
        let sharedSecret = try privateKey.sharedSecretFromKeyAgreement(with: peerPublicKey)
        let symmetricKey = sharedSecret.hkdfDerivedSymmetricKey(
            using: SHA256.self,
            salt: Data(),
            sharedInfo: SpanProtocolV1.textKeyInfo,
            outputByteCount: 32
        )

        var nonceData = Data(count: 12)
        let status = nonceData.withUnsafeMutableBytes { buffer in
            SecRandomCopyBytes(kSecRandomDefault, buffer.count, buffer.baseAddress!)
        }
        guard status == errSecSuccess else { throw SpanCryptoError.randomFailed }

        let nonce = try ChaChaPoly.Nonce(data: nonceData)
        let sealed = try ChaChaPoly.seal(Data(text.utf8), using: symmetricKey, nonce: nonce)
        var ciphertextAndTag = Data()
        ciphertextAndTag.append(sealed.ciphertext)
        ciphertextAndTag.append(sealed.tag)

        return (Hex.encode(nonceData), Hex.encode(ciphertextAndTag))
    }
}

enum SpanCryptoError: Error {
    case badPrivateKey
    case badPeerPublicKey
    case randomFailed
}
