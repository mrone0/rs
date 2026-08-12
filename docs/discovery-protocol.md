# 设备发现协议

当前 PC MVP 使用一个极轻量的自定义 UDP 广播协议，不使用 Flutter、Electron、数据库或常驻 Web runtime。

## 选择

- 发现协议：自定义 UDP broadcast
- 发现端口：`46792/UDP`
- 文本传输：`46793/TCP`（按需建立短连接，不是 TCP 长连接）
- 数据格式：TSV 文本行
- 范围：同一局域网

第一版先选 UDP broadcast，而不是 mDNS，原因是：

- 依赖更少
- 二进制更小
- 实现更直观
- 适合先验证 Windows ⇄ macOS PC 同步

后续如果要更标准的零配置发现，可以把发现层替换为 mDNS/Bonjour，但核心信任列表和文本传输不用重写。

## Discovery Packet

广播 payload：

```text
SPAN_DISCOVERY_V2\t<device_id>\t<device_name>\t<platform>
```

示例：

```text
SPAN_DISCOVERY_V2\tmacbook-1723281000000\tMacBook\tmacos
```

字段：

| 字段 | 说明 |
|---|---|
| `magic` | 固定为 `SPAN_DISCOVERY_V2` |
| `device_id` | 本机生成并保存的设备 ID |
| `device_name` | 本机设备名 |
| `platform` | `windows` / `macos` / `linux` |

## Trust

发现只负责“看见设备”，不等于允许接收剪贴板。

```text
Discovered → Trusted → Revoked
```

只有 `Trusted` 且保存了 `host` 的设备会收到自动广播。daemon 运行时会监听可信设备公告，并把 UDP 源 IP 写回该设备的 `host`，所以设备换 IP 后不需要重新信任。

接收端也会校验 `from_device_id` 是否仍在本机的 `Trusted` 列表里；不可信发送方的 TCP 文本会直接丢弃。

文本载荷现在使用 `ChaCha20-Poly1305` 加密，密钥由本机私钥和对端公钥经 `X25519 + HKDF-SHA256` 派生。

## 后台资源占用

这里的 UDP 不是“保持一条连接”：UDP 无连接状态。daemon 只保留一个阻塞式 UDP socket，空闲时线程睡在内核的 `recv_from` 上，不会高频轮询 CPU；每 15 秒发送一个很小的公告包。剪贴板正文不会走 UDP 广播，而是只对每个已信任设备按需建立 TCP 短连接并发送一次。

因此常驻占用主要来自系统剪贴板监听和 Rust 进程本身，而不是网络连接。当前设计没有 Tokio、Electron、WebView 或数据库；正常空闲时 CPU 应接近 0，内存主要是进程基础开销。局域网设备很多时可以把公告间隔调高，但 V1 的 15 秒有利于设备换 IP 后快速恢复。

## Text Packet

文本传输走 TCP，payload：

```text
SPAN_TEXT_V3\t<from_device_id>\t<byte_len>\n<utf8_text>
```

限制：

- 第一版只传 UTF-8 文本
- 单条文本最大 `64 KiB`
- 收到后直接写入系统剪贴板
- 远端写入不会回环广播

## 安全边界

当前 MVP 已经做了加密与可信设备校验，但仍建议只在可信局域网内使用。

正式公开版本后续可继续补强：

- 设备指纹
- 首次信任确认
- 会话密钥轮换
- 消息签名审计
- 防重放 nonce
