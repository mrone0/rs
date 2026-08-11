import Foundation

struct LocalSpanIdentity: Codable, Hashable {
    let id: String
    var name: String
    let privateKeyHex: String
    let publicKeyHex: String
}
