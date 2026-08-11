import Foundation

struct SpanTextPayload: Codable, Hashable {
    let fromDeviceID: String
    let text: String
    let createdAt: Date
}
