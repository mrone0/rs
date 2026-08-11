# Span iOS TestFlight

目标：把 `apps/ios/Span.xcodeproj` 归档并上传到 App Store Connect，然后通过 TestFlight 安装测试。

## 当前 iOS 功能

- App 主界面发现局域网设备
- 本机身份生成：Curve25519 private/public key
- UDP discovery：`SPAN_DISCOVERY_V2` / `46792`
- TCP 文本发送：`SPAN_TEXT_V3` / `46793`
- 加密：`X25519 + HKDF-SHA256 + ChaCha20-Poly1305`
- 可信设备列表：App 和 Share Extension 通过 App Group 共享
- 一键发送当前剪贴板
- Shortcuts：`Send Clipboard to Span`
- Share Extension：从分享菜单发送选中文本/URL
- 手动配对：可输入 PC 的 device id、IP、公钥

## Apple Developer 配置

在 Apple Developer / Xcode 中创建并保持一致：

| 项 | 默认值 | 说明 |
|---|---|---|
| App Bundle ID | `app.span.ios` | 可改成你的正式 ID |
| Share Extension Bundle ID | `app.span.ios.ShareExtension` | 必须以 App Bundle ID 为前缀 |
| App Group | `group.app.span.ios` | App 和 Share Extension 必须同时开启 |

如果你改 Bundle ID / App Group，需要同步修改：

- `apps/ios/Span.xcodeproj`
- `apps/ios/Span/Span.entitlements`
- `apps/ios/ShareExtension/ShareExtension.entitlements`
- `apps/ios/Span/SpanAppGroup.swift`

## 本地验证

```sh
./scripts/ios-typecheck.sh
```

## 本地归档并导出 IPA

先在 Xcode 安装 iOS platform，然后登录 Apple ID，设置 Team / Bundle ID / App Group。

```sh
./scripts/ios-archive.sh
```

导出位置：

```text
build/ios/export
```

## 上传 TestFlight

最简单方式：

1. Xcode 打开 `apps/ios/Span.xcodeproj`
2. Product → Archive
3. Organizer → Distribute App
4. App Store Connect → Upload
5. App Store Connect → TestFlight 中等待处理完成
6. 添加内部测试员并安装

CLI 上传也可用：

```sh
xcrun altool --upload-app \
  --type ios \
  -f build/ios/export/Span.ipa \
  --api-key "$APP_STORE_CONNECT_API_KEY_ID" \
  --api-issuer "$APP_STORE_CONNECT_ISSUER_ID" \
  --p8-file-path AuthKey_${APP_STORE_CONNECT_API_KEY_ID}.p8
```

## 与 PC 端测试

PC 端启动：

```sh
span install
span start
span status
```

`span status` 会输出 PC 的 `device` 和 `public key`，iOS 手动配对时需要用到。

PC 信任 iPhone：

```sh
span discover
span trust <ios_device_id> <ios_name> ios <ios_ip>
```

之后 iPhone 端可以通过 App / Shortcuts / Share Extension 把文本发到 PC 剪贴板。
