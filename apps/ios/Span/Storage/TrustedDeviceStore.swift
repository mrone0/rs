import Foundation

protocol TrustedDeviceStore {
    func loadDevices() -> [SpanDevice]
    func saveDevices(_ devices: [SpanDevice])
}

final class UserDefaultsTrustedDeviceStore: TrustedDeviceStore {
    private let key = "span.trusted.devices.v1"
    private let defaults: UserDefaults

    init(defaults: UserDefaults = SpanAppGroup.defaults) {
        self.defaults = defaults
    }

    func loadDevices() -> [SpanDevice] {
        guard let data = defaults.data(forKey: key) else { return [] }
        return (try? JSONDecoder().decode([SpanDevice].self, from: data)) ?? []
    }

    func saveDevices(_ devices: [SpanDevice]) {
        let data = try? JSONEncoder().encode(devices)
        defaults.set(data, forKey: key)
    }
}
