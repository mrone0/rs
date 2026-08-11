# 发布与打包

PC 端不要求用户本地构建，直接用 GitHub Actions 产物。

## 自动打包

`.github/workflows/release.yml` 会构建：

- `rs-cp-macos-arm64.tar.gz`
- `rs-cp-macos-x64.tar.gz`
- `rs-cp-windows-x64.zip`
- `rs-cp-linux-x64.tar.gz`

触发方式：

```sh
git tag v0.1.0
git push origin v0.1.0
```

也可以在 GitHub Actions 页面手动点 `workflow_dispatch`。

## 包内容

每个包包含：

- `rs-cp-daemon` / `rs-cp-daemon.exe`
- `README.md`
- `pc-mvp.md`
- `discovery-protocol.md`

## 本地验证

```sh
cargo test --workspace
cargo build --release -p rs-cp-daemon
./target/release/rs-cp-daemon ui
```

## 体积目标

当前 macOS release 二进制约 `346K`。

保持体积小的原则：

- PC 端不使用 Flutter/Electron
- 不内置 WebView
- 不引入数据库
- 不引入异步 runtime，除非确实需要
- UI 只保留 CLI/托盘/系统服务入口

## 开源前需要确认

公开仓库前建议补：

- `LICENSE`：建议 MIT 或 Apache-2.0
- `SECURITY.md`：说明当前是否加密、如何报告漏洞
- `CONTRIBUTING.md`：说明如何跑测试和构建
- Release notes：说明当前已加密，但仍建议仅在可信局域网使用
