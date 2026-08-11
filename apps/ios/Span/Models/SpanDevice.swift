import Foundation

struct SpanDevice: Identifiable, Hashable, Codable {
    let id: String
    var name: String
    var platform: String
    var host: String?
    var publicKeyHex: String?
    var isTrusted: Bool
    var lastSeenAt: Date?
}
