# PC MVP

目标：先完成 `Windows ⇄ macOS` 文本剪贴板同步，后续扩展到更广义的跨设备数据互通。PC 端由无窗口 daemon 低占用常驻，并提供轻量原生 GUI 管理设备。

## 为什么不用 Flutter / Electron

普通用户需要 GUI，但不能为一个设备管理窗口捆绑大型 runtime。当前选择：

- Rust 单二进制
- macOS 使用 AppKit 原生 API
- Windows 使用 Win32 原生 API
- 不使用 Electron、Flutter、WebView、GTK runtime
- 无数据库
- 无 Tokio 等异步运行时
- 仅 Windows 目标引入 `windows-sys`

macOS 当前 release 二进制约 `346K`。

## 普通用户界面

```text
打开 span
  ├─ Discover：扫描同一局域网，发现新设备后弹窗确认配对
  └─ Remove selected：删除选中的受信任设备
```

配对完成后，后台 daemon 自动监听系统剪贴板。用户只需要使用系统自己的 `Ctrl/Cmd+C` 和 `Ctrl/Cmd+V`；每次复制会自动发送给全部受信任设备，不需要点击 Send。

GUI 会自动安装登录自启并启动 daemon。关闭 GUI 只关闭管理窗口，不会停止后台同步。

## CLI（调试/脚本）

```sh
span install             # 安装并启用登录自启
span start               # 启动后台同步
span stop                # 停止后台同步
span restart             # 重启后台同步
span discover            # 立即查找局域网设备
span accept [编号]       # 兼容调试：接受配对
span send [文本]         # 兼容调试：发送文本；无参数时发送当前剪贴板
```

后台 daemon 每 15 秒发送一个很小的 UDP 设备公告，并阻塞监听发现请求；UDP 公告只包含设备元数据，不包含剪贴板正文。

`span install` 会注册并立即启动系统自启服务：

- macOS：LaunchAgent
- Windows：用户登录任务
- Linux：桌面自启动项

配对完成后，本机剪贴板变化会自动加密发送给所有已接受设备；收到远端文本后会写入系统剪贴板，并抑制回环广播。

开发调试所需的旧子命令仍保持兼容，但不出现在普通 `span --help` 中。

## 当前限制

- 文本传输已加密，但仍只面向可信局域网
- macOS 剪贴板读写已接原生 `NSPasteboard`，并使用 `changeCount` 降低轮询成本
- Windows 剪贴板读写已接原生 Win32 API，并使用 `GetClipboardSequenceNumber` 降低读取成本


## 系统剪贴板验证

Span daemon 运行且两边都接受配对后，不需要 Span 专用复制命令。直接在任意应用中复制：

```text
电脑 A：Command/Ctrl+C
电脑 B：Command/Ctrl+V
```

`span send` 可用于立即重发当前剪贴板；V1 只同步纯文本。
