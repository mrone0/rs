import Social
import UIKit
import UniformTypeIdentifiers

final class ShareViewController: UIViewController {
    private let statusLabel = UILabel()

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        statusLabel.translatesAutoresizingMaskIntoConstraints = false
        statusLabel.textAlignment = .center
        statusLabel.numberOfLines = 0
        statusLabel.text = "Sending with Span…"
        view.addSubview(statusLabel)
        NSLayoutConstraint.activate([
            statusLabel.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 24),
            statusLabel.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -24),
            statusLabel.centerYAnchor.constraint(equalTo: view.centerYAnchor)
        ])
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        Task { await sendSharedText() }
    }

    private func sendSharedText() async {
        do {
            guard let text = await extractText(), !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                finish(message: "No text found")
                return
            }

            let dispatcher = SpanDispatcher()
            let sent = try await dispatcher.sendText(text)
            finish(message: sent == 0 ? "No trusted devices" : "Sent to \(sent) device(s)")
        } catch {
            finish(message: "Send failed")
        }
    }

    private func extractText() async -> String? {
        guard let items = extensionContext?.inputItems as? [NSExtensionItem] else { return nil }
        for item in items {
            guard let providers = item.attachments else { continue }
            for provider in providers {
                if provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier),
                   let value = await loadString(from: provider, typeIdentifier: UTType.plainText.identifier) {
                    return value
                }
                if provider.hasItemConformingToTypeIdentifier(UTType.text.identifier),
                   let value = await loadString(from: provider, typeIdentifier: UTType.text.identifier) {
                    return value
                }
                if provider.hasItemConformingToTypeIdentifier(UTType.url.identifier),
                   let value = await loadURLString(from: provider) {
                    return value
                }
            }
        }
        return nil
    }

    private func loadString(from provider: NSItemProvider, typeIdentifier: String) async -> String? {
        await withCheckedContinuation { continuation in
            provider.loadItem(forTypeIdentifier: typeIdentifier, options: nil) { item, _ in
                if let string = item as? String {
                    continuation.resume(returning: string)
                } else if let data = item as? Data {
                    continuation.resume(returning: String(data: data, encoding: .utf8))
                } else if let url = item as? URL {
                    continuation.resume(returning: url.absoluteString)
                } else {
                    continuation.resume(returning: nil)
                }
            }
        }
    }

    private func loadURLString(from provider: NSItemProvider) async -> String? {
        await withCheckedContinuation { continuation in
            provider.loadItem(forTypeIdentifier: UTType.url.identifier, options: nil) { item, _ in
                if let url = item as? URL {
                    continuation.resume(returning: url.absoluteString)
                } else if let string = item as? String {
                    continuation.resume(returning: string)
                } else {
                    continuation.resume(returning: nil)
                }
            }
        }
    }

    @MainActor
    private func finish(message: String) {
        statusLabel.text = message
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { [weak self] in
            self?.extensionContext?.completeRequest(returningItems: nil)
        }
    }
}
