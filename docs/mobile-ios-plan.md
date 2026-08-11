# iOS App 计划

iOS 端单独做，不并进 PC daemon。第一版目标不是后台监听，而是系统允许的一键触发。

## 现实边界

- iOS 不能长期后台无感监听系统剪贴板
- iOS 端必须由用户触发读取剪贴板或分享文本
- PC 端负责常驻接收并写入系统剪贴板

## V1 入口

优先级：

1. Shortcuts Action：读取当前剪贴板并发送到可信 PC
2. Share Extension：选中文本后分享给 `rs-cp`
3. App 主界面：发现设备、信任设备、移除设备
4. Back Tap / Action Button：绑定快捷指令触发

## 不上 TestFlight 的安装方式

如果暂时不走 TestFlight，可以先用：

- Xcode 本机真机运行
- Apple Developer 个人签名
- 内部自用的手动安装流程

## 与 PC 端协议

- 设备发现：兼容 `46792/UDP`，或 App 内手动填 IP
- 文本发送：兼容 `46793/TCP`
- 信任列表：iOS 本地保存已信任 PC
- 发送策略：默认广播到所有已信任 PC

## 重点体验

```text
iPhone 复制文本
→ 点快捷指令 / 分享扩展
→ PC 端自动写入剪贴板
→ 电脑直接 Ctrl+V / Cmd+V
```
