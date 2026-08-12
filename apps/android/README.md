# Span Android

Android 端第一版只做**纯文本双向流转**，目标是用最小的系统开销把手机和同一局域网内已信任的 Span PC 连接起来。

## 已支持

- Android 10+（`minSdk 29`）
- 当前剪贴板一键发送
- 系统分享菜单发送选中的文本 / URL
- Quick Settings Tile 一键发送当前剪贴板
- PC → Android：前台接收服务监听 TCP 46793，解密后写入系统剪贴板
- 开机广播恢复接收服务（用户关闭接收后不会恢复）
- 局域网 UDP 自动发现
- 设备信任 / 撤销
- `X25519 + HKDF-SHA256 + ChaCha20-Poly1305`，与 Rust PC 端协议兼容
- 原生 Android View，无 Compose、无第三方运行时

## 当前边界

- Android 端现在支持双向文本链路：Android → PC 主动发送，PC → Android 由接收服务写入剪贴板。
- Android 10 以后系统限制后台读取剪贴板。Span 不做绕过系统限制的常驻读取：
  - 打开 Span 后可读取当前剪贴板；
  - Quick Settings Tile 会短暂拉起 Span，再读取并发送；
  - 系统分享菜单是最可靠的选中文本入口。
- PC → Android 接收是独立的前台服务，只接收已信任设备的加密 TCP 文本，不读取手机当前剪贴板。

## 本机编译

需要 Android SDK、JDK 21 和 Android 36 平台：

```sh
cd apps/android
JAVA_HOME=$(/usr/libexec/java_home -v 21) ./gradlew :app:testDebugUnitTest :app:assembleDebug
```

APK：

```text
apps/android/app/build/outputs/apk/debug/app-debug.apk
apps/android/app/build/outputs/apk/release/app-release-unsigned.apk
```

本机实测体积：debug 约 80K，R8 + 资源压缩后的 release unsigned 约 28K。release 还没有配置发布签名，真机临时安装优先使用 debug APK。

`local.properties` 只用于本机 Android SDK 路径，已加入根目录 `.gitignore`。

## 与 PC 配对

PC 端先启动后台守护进程：

```sh
span start
```

Android 打开 Span 后会自动发现局域网设备。PC 端也可以执行：

```sh
span discover
```

发现 Android 后，PC 端可直接信任已发现设备：

```sh
span trust <android_device_id>
```

Android 端在 Devices 列表中点击 PC 的 **Trust**。手动配对时需要填写：

- Device ID
- Host / IP
- 32 字节 X25519 公钥十六进制字符串

信任关系保存在本地；未信任设备不会收到文本广播。

## GitHub Actions

`.github/workflows/android-apk.yml` 会在 Android 代码变更或手动触发时：

1. 安装 JDK 21 和 Android 36 SDK；
2. 运行 JVM 单元测试；
3. 构建 debug 和 release APK；
4. 上传 `span-android-debug-apk` artifact，其中同时包含 debug APK 和 release unsigned APK。
