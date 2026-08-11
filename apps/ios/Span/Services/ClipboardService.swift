import Foundation

protocol ClipboardService: Sendable {
    @MainActor func currentText() -> String?
    @MainActor func setText(_ text: String)
}

#if APP_EXTENSION
final class SystemClipboardService: ClipboardService {
    @MainActor func currentText() -> String? { nil }
    @MainActor func setText(_ text: String) {}
}
#else
import UIKit

final class SystemClipboardService: ClipboardService {
    @MainActor func currentText() -> String? {
        UIPasteboard.general.string
    }

    @MainActor func setText(_ text: String) {
        UIPasteboard.general.string = text
    }
}
#endif
