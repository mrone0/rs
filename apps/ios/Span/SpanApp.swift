import SwiftUI

@main
struct SpanApp: App {
    @StateObject private var model = SpanViewModel()

    var body: some Scene {
        WindowGroup {
            ContentView(model: model)
        }
    }
}
