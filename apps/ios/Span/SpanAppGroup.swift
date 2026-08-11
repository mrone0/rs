import Foundation

enum SpanAppGroup {
    // Change this together with Span.entitlements and ShareExtension.entitlements
    // when you set your own Apple Developer Team / Bundle ID.
    static let identifier = "group.app.span.ios"

    static var defaults: UserDefaults {
        UserDefaults(suiteName: identifier) ?? .standard
    }
}
