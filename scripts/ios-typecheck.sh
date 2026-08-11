#!/usr/bin/env bash
set -euo pipefail
SDK="$(xcrun --sdk iphonesimulator --show-sdk-path)"
swiftc -typecheck -sdk "$SDK" -target arm64-apple-ios17.0-simulator \
  apps/ios/Span/*.swift \
  apps/ios/Span/Models/*.swift \
  apps/ios/Span/Protocol/*.swift \
  apps/ios/Span/Services/*.swift \
  apps/ios/Span/Storage/*.swift \
  apps/ios/Span/Utilities/*.swift
swiftc -typecheck -D APP_EXTENSION -sdk "$SDK" -target arm64-apple-ios17.0-simulator \
  apps/ios/ShareExtension/*.swift \
  apps/ios/Span/SpanAppGroup.swift \
  apps/ios/Span/Models/*.swift \
  apps/ios/Span/Protocol/*.swift \
  apps/ios/Span/Services/*.swift \
  apps/ios/Span/Storage/*.swift \
  apps/ios/Span/Utilities/*.swift
xcodebuild -list -project apps/ios/Span.xcodeproj >/dev/null
