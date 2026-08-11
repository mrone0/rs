# 设备发现协议

当前 PC MVP 使用一个极轻量的自定义 UDP 广播协议，不使用 Flutter、Electron、数据库或常驻 Web runtime。

## 选择

- 发现协议：自定义 UDP broadcast
- 发现端口：`46792/UDP`
- 文本传输：`46793/TCP`
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
RSCP_DISCOVERY_V1\t<device_id>\t<device_name>\t<platform>
```

示例：

```text
RSCP_DISCOVERY_V1\tmacbook-1723281000000\tMacBook\tmacos
```

字段：

| 字段 | 说明 |
|---|---|
| `magic` | 固定为 `RSCP_DISCOVERY_V1` |
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

文本载荷现在使用 `XChaCha20-Poly1305` 加密，密钥由本机私钥和对端公钥经 `X25519 + HKDF-SHA256` 派生。

## Text Packet

文本传输走 TCP，payload：

```text
RSCP_TEXT_V1\t<from_device_id>\t<byte_len>\n<utf8_text>
```

限制：

- 第一版只传 UTF-8 文本
- 单条文本最大 `64 KiB`
- 收到后直接写入系统剪贴板
- 远端写入不会回环广播

## 安全边界

当前 MVP 还没有加密和身份签名，只适合本地开发/可信局域网验证。

正式公开版本需要补：

- 设备指纹
- 首次信任确认
- 会话密钥
- 消息签名或 AEAD 加密
- 防重放 nonce
