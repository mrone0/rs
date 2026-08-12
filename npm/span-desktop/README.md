# span-desktop

Install the Span low-footprint desktop daemon from npm. The package exposes a
single `span` command and downloads the matching Rust binary for macOS, Windows,
or Linux during installation.

## Install from npm

```sh
npm install -g span-desktop
span
span install
```

`span install` registers and starts the daemon so it runs in the background
without keeping a terminal window open. Run `span` (or `span gui`) for the lightweight native device
manager. The public CLI stays intentionally small:

```text
span install
span start
span stop
span restart
span discover
span accept [number]
span send [text]
```

## Local package test

Build the Rust CLI and GUI binaries, then install a local npm tarball using them:

```sh
cargo build --release -p span
cd npm/span-desktop
npm pack
SPAN_LOCAL_BINARY="$PWD/../../target/release/span" \
  npm install -g ./span-desktop-0.1.2.tgz
span --help
span
span install
```

`SPAN_LOCAL_BINARY` is only for local development. A normal npm installation
downloads the matching GitHub Release asset. Each desktop archive contains both `span` (CLI + daemon) and `span-gui` (native GUI). Running `span` with no arguments opens the GUI; CLI commands are forwarded to `span`.

## Environment variables

- `SPAN_LOCAL_BINARY` / `SPAN_LOCAL_GUI_BINARY`: use locally built CLI and GUI binaries instead of downloading.
- `SPAN_VERSION`: override the GitHub Release version, for example `0.1.2`.
- `SPAN_REPOSITORY`: override the GitHub repository, default `mrone0/rs`.
- `SPAN_RELEASE_BASE_URL`: override the release asset base URL.
- `SPAN_SHA256`: optionally verify the downloaded archive checksum.
- `SPAN_SKIP_DOWNLOAD=1`: skip binary download during package development.

## Supported desktop targets

- macOS Apple Silicon (`darwin/arm64`)
- macOS Intel (`darwin/x64`)
- Linux x64 (`linux/x64`)
- Windows x64 (`win32/x64`)
