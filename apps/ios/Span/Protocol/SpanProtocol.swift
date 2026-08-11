import Foundation

enum SpanProtocolV1 {
    static let discoveryPort: UInt16 = 46792
    static let textPort: UInt16 = 46793
    static let discoveryMagic = "SPAN_DISCOVERY_V2"
    static let textMagic = "SPAN_TEXT_V3"
    static let textKeyInfo = Data("span-text-v3".utf8)
    static let maxTextBytes = 64 * 1024
}
