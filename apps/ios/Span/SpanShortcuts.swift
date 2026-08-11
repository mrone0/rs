import AppIntents
import Foundation

struct SendClipboardIntent: AppIntent {
    static var title: LocalizedStringResource = "Send Clipboard to Span"
    static var description = IntentDescription("Send the current clipboard text to trusted devices on the local network.")
    static var openAppWhenRun = false

    @MainActor
    func perform() async throws -> some IntentResult & ProvidesDialog {
        let dispatcher = SpanDispatcher()
        let sent = try await dispatcher.sendClipboard()
        return .result(dialog: sent == 0 ? "Clipboard is empty or no trusted devices." : "Sent to \(sent) device(s).")
    }
}

struct SendTextIntent: AppIntent {
    static var title: LocalizedStringResource = "Send Text to Span"
    static var description = IntentDescription("Send the provided text to trusted devices on the local network.")
    static var openAppWhenRun = false

    @Parameter(title: "Text")
    var text: String

    static var parameterSummary: some ParameterSummary {
        Summary("Send \(\.$text) with Span")
    }

    @MainActor
    func perform() async throws -> some IntentResult & ProvidesDialog {
        let dispatcher = SpanDispatcher()
        let sent = try await dispatcher.sendText(text)
        return .result(dialog: sent == 0 ? "No trusted devices available." : "Sent to \(sent) device(s).")
    }
}

struct SpanShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: SendClipboardIntent(),
            phrases: ["Send clipboard with \(.applicationName)", "Share clipboard in \(.applicationName)"],
            shortTitle: "Send Clipboard",
            systemImageName: "doc.on.clipboard"
        )
    }
}
