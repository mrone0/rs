# Span iOS 端

第一版目标：

- 一键发送当前剪贴板
- 选中文本后通过 Share Extension 发送
- 设备发现 / 信任 / 移除
- 仅文本

## 约束

- 不做 iOS 后台无感监听剪贴板
- 不依赖重型运行时
- 以系统允许的触发入口为主

## 当前实现

- SwiftUI 主界面
- 本地设备身份：`Curve25519` 私钥 / 公钥，保存在 `UserDefaults`
- UDP 发现：广播 / 监听 `SPAN_DISCOVERY_V2`，启动后每 15 秒公告一次
- TCP 发送：兼容 PC 端 `SPAN_TEXT_V3`
- 加密：`X25519 + HKDF-SHA256 + ChaCha20-Poly1305`
- 信任设备：本地保存，默认只发给 trusted devices

## 计划中的入口

1. App 主界面：管理设备
2. Shortcuts / Action：发送当前剪贴板
3. Share Extension：发送选中文本
4. 可选：Back Tap 绑定快捷指令

## 目录

- `Span.xcodeproj`：可用 Xcode 打开的工程，包含主 App 和 Share Extension target
- `Span/SpanApp.swift`：应用入口
- `Span/ContentView.swift`：主界面
- `Span/SpanViewModel.swift`：状态与操作
- `Span/Models/`：设备、身份、文本消息
- `Span/Protocol/`：端口 / magic / key info
- `Span/Services/`：发现、传输、剪贴板、加密
- `Span/Storage/`：本地身份和可信设备存储
- `Span/Utilities/`：通用工具

## TestFlight

详见：`../../docs/ios-testflight.md`。

## 本地验证

```sh
SDK=$(xcrun --sdk iphonesimulator --show-sdk-path)
swiftc -typecheck -sdk "$SDK" -target arm64-apple-ios17.0-simulator \
  apps/ios/Span/*.swift \
  apps/ios/Span/Models/*.swift \
  apps/ios/Span/Protocol/*.swift \
  apps/ios/Span/Services/*.swift \
  apps/ios/Span/Storage/*.swift \
  apps/ios/Span/Utilities/*.swift
```

## 备注

iOS 端会单独做成一个 app，不并进 PC daemon。真机运行需要在 Xcode 中配置 Team / Bundle ID，并允许 Local Network 权限。
## 当前本机验证结果

已验证以下命令可以通过 Swift 6 编译：

```sh
# 类型检查
./scripts/ios-typecheck.sh

# iOS Simulator 构建（不签名）
xcodebuild -project apps/ios/Span.xcodeproj \
  -scheme Span -configuration Debug \
  -destination 'generic/platform=iOS Simulator' \
  build CODE_SIGNING_ALLOWED=NO

# iOS 真机架构构建（不签名；用于确认 iphoneos 代码可编译）
xcodebuild -project apps/ios/Span.xcodeproj \
  -scheme Span -configuration Debug \
  -destination 'generic/platform=iOS' \
  build CODE_SIGNING_ALLOWED=NO
```

> 上面的 `CODE_SIGNING_ALLOWED=NO` 只能验证代码和真机架构，不能把 App 安装到 iPhone。真机 Run 仍需要在 Xcode 的 Signing & Capabilities 中选择 Team，并为主 App、Share Extension 配置唯一 Bundle ID。当前工程还使用 App Group `group.app.span.ios`；如果只是用个人 Apple ID 做最简单的主 App 验证，可先暂时关闭 Share Extension 和 App Group。
