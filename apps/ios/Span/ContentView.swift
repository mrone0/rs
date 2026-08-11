import SwiftUI

struct ContentView: View {
    @ObservedObject var model: SpanViewModel

    var body: some View {
        NavigationStack {
            List {
                Section("Quick Send") {
                    Button("Send Clipboard") {
                        model.sendCurrentClipboard()
                    }
                    Button("Send Selection") {
                        model.sendCurrentSelection()
                    }
                }

                Section("Network") {
                    Button("Start Discovery") { model.start() }
                    Button("Announce Once") { model.announce() }
                    Button("Stop Discovery") { model.stop() }
                }

                Section("This Device") {
                    Text(model.localSummary)
                        .font(.caption)
                        .textSelection(.enabled)
                    Text("Use this id/public key when trusting the iPhone from PC.")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }

                Section("Manual Pair") {
                    TextField("Device ID", text: $model.manualDeviceID)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    TextField("Name", text: $model.manualName)
                    TextField("Host / IP", text: $model.manualHost)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    TextField("Public Key Hex", text: $model.manualPublicKeyHex)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    Button("Trust Manual Device") {
                        model.addManualDevice()
                    }
                }

                Section("Trusted Devices") {
                    if model.devices.isEmpty {
                        Text("No devices yet")
                            .foregroundStyle(.secondary)
                    } else {
                        ForEach(model.devices) { device in
                            VStack(alignment: .leading, spacing: 6) {
                                HStack {
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(device.name)
                                        Text(device.platform)
                                            .font(.caption)
                                            .foregroundStyle(.secondary)
                                    }
                                    Spacer()
                                    Text(device.isTrusted ? "Trusted" : "Discovered")
                                        .font(.caption)
                                        .foregroundStyle(device.isTrusted ? .green : .secondary)
                                }

                                if let host = device.host {
                                    Text(host)
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }

                                if let key = device.publicKeyHex {
                                    Text("key: \(key.prefix(16))…")
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }

                                HStack {
                                    Button(device.isTrusted ? "Revoke" : "Trust") {
                                        if device.isTrusted {
                                            model.revoke(deviceID: device.id)
                                        } else {
                                            model.trust(deviceID: device.id)
                                        }
                                    }
                                    .buttonStyle(.bordered)
                                }
                            }
                            .padding(.vertical, 4)
                        }
                    }
                }

                Section("Status") {
                    Text(model.statusMessage)
                        .font(.callout)
                }
            }
            .navigationTitle("Span")
        }
    }
}
