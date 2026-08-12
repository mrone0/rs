# span-desktop

Install the Span low-footprint desktop daemon from npm. The package exposes a
single `span` command and downloads the matching Rust binary for macOS, Windows,
or Linux during installation.

## Install from npm

```sh
npm install -g span-desktop
span install
```

`span install` registers the daemon with the operating system so it runs in the
background without keeping a terminal window open.

## Local package test

Build the Rust binary, then install a local npm tarball using that binary:

```sh
cargo build --release -p span
cd npm/span-desktop
npm pack
SPAN_LOCAL_BINARY="$PWD/../../target/release/span" \
  npm install -g ./span-desktop-0.1.2.tgz
span status
span install
```

`SPAN_LOCAL_BINARY` is only for local development. A normal npm installation
downloads the matching GitHub Release asset.

## Environment variables

- `SPAN_LOCAL_BINARY`: use a locally built binary instead of downloading.
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
