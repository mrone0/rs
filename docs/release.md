# 发布与打包

PC 端不要求用户本地构建，直接用 GitHub Actions 产物。当前第一版仍以文本同步为主，后续可扩展为更广义的数据互通。

## 自动打包

`.github/workflows/release.yml` 会构建：

- `span-macos-arm64.tar.gz`
- `span-macos-x64.tar.gz`
- `span-windows-x64.zip`
- `span-linux-x64.tar.gz`

触发方式：

```sh
git tag v0.1.0
git push origin v0.1.0
```

也可以在 GitHub Actions 页面手动点 `workflow_dispatch`。

## 包内容

每个包包含：

- `span` / `span.exe`
- `README.md`
- `pc-mvp.md`
- `discovery-protocol.md`

## 本地验证

```sh
cargo test --workspace
cargo build --release -p span
./target/release/span ui
```

## 体积目标

当前 release 二进制仍保持在几百 KB 量级。

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

## npm 桌面端安装包

目录：`npm/span-desktop`。

npm 包的职责只有两件事：

1. 安装时识别 `macOS arm64/x64`、`Windows x64` 或 `Linux x64`。
2. 下载对应 GitHub Release 压缩包中的 `span` 二进制，并提供 `span` 命令。

因此 npm 层不会引入 Electron，实际常驻进程仍然是 Rust daemon。

### 本地打包测试

```sh
cargo build --release -p span
cd npm/span-desktop
npm pack
SPAN_LOCAL_BINARY="$PWD/../../target/release/span" \
  npm install --prefix /tmp/span-npm-prefix ./span-desktop-0.1.2.tgz
/tmp/span-npm-prefix/node_modules/.bin/span status
```

### npm 发布

首次发布前确认 npm 包名未被占用，并在本地登录。由于 npm 的可写 Granular Access Token 目前最多 90 天，建议只用它完成首次初始化发布，随后切换到 Trusted Publishing（GitHub Actions OIDC）：

```sh
npm login
npm whoami
cd npm/span-desktop
npm version 0.1.2-test.0 --no-git-tag-version
npm publish --access public --tag test
```

发布后用户可以直接：

```sh
npm install -g span-desktop
span install
```

### GitHub Actions 自动发布（Trusted Publishing）

首次发布成功后，在 npm 包 `span-desktop` 的设置中添加 Trusted Publisher：

```text
Provider: GitHub Actions
Owner: mrone0
Repository: rs
Workflow: release.yml
Environment: 留空
```

然后在 GitHub Actions workflow 的 npm 发布 job 中启用：

```yaml
permissions:
  contents: read
  id-token: write
```

当前 `release.yml` 已配置 OIDC，不再需要保存 `NPM_TOKEN`。预发布 tag（例如 `v0.1.2-test.0`）进入 npm `test` 通道；正式 tag（例如 `v0.1.2`）进入 `latest`。

如果暂时不用 Trusted Publishing，也可以使用短期 Token。GitHub 仓库需要添加：

```text
NPM_TOKEN
```

同一版本不能重复发布；例如 `v0.1.2` 对应 `span-desktop@0.1.2`。测试版本对应关系为：`v0.1.2-test.0` → `span-desktop@0.1.2-test.0`。
