# PC MVP

目标：先完成 `Windows ⇄ macOS` 文本剪贴板同步，后续扩展到更广义的跨设备数据互通。PC 端无主界面、低占用、登录自启。

## 为什么不用 Flutter / Electron

PC 端第一版不需要主界面。为了包体积和内存占用最小，当前选择：

- Rust 单二进制
- 无 GUI runtime
- 无数据库
- 无 Tokio 等异步运行时
- 仅 Windows 目标引入 `windows-sys`

macOS 当前 release 二进制约 `346K`。

## 命令

```sh
cargo build --release
./target/release/span status
./target/release/span ui
```

## 局域网发现

一台机器监听：

```sh
./target/release/span discover
```

另一台机器广播：

```sh
./target/release/span announce
```

## 信任设备

```sh
./target/release/span trust <id>
./target/release/span trust <id> <name> <platform> [host] [public-key]
./target/release/span devices
```

示例：

```sh
./target/release/span trust macbook-local MacBook macos 192.168.1.23
```

`span discover` 会把发现到的设备、IP 和公钥保存为 discovered 记录；随后直接执行 `span trust <id>` 即可信任它。

只有 `Trusted` 且带 `host`、公钥的设备会收到自动广播。`host` 和公钥也可以手动填；后台运行后会根据可信设备的发现广播自动刷新 IP，但公钥变化会被拒绝，避免设备身份被静默替换。

## 后台同步

两台 PC 都运行：

```sh
./target/release/span start
```

行为：

- 接收端监听 `46793/TCP`
- 后台监听 `46792/UDP`，自动刷新可信设备 IP
- 本机剪贴板变化后发送到所有可信设备
- 收到远端文本后写入系统剪贴板
- 收到远端写入后不回环广播

## 登录自启

```sh
./target/release/span install
./target/release/span uninstall
```

实现：

- macOS：`~/Library/LaunchAgents/com.span.daemon.plist`
- Windows：用户启动目录 `span.cmd`
- Linux：`~/.config/autostart/span.desktop`

## 当前限制

- 文本传输已加密，但仍只面向可信局域网
- macOS 剪贴板读写已接原生 `NSPasteboard`，并使用 `changeCount` 降低轮询成本
- Windows 剪贴板读写已接原生 Win32 API，并使用 `GetClipboardSequenceNumber` 降低读取成本
