#!/usr/bin/env bash
set -euo pipefail

# Before running:
# 1. Open apps/ios/Span.xcodeproj in Xcode.
# 2. Set Team, Bundle ID, and App Group to values registered in Apple Developer.
# 3. Ensure the iOS platform is installed in Xcode Settings > Components.

ARCHIVE_PATH="${ARCHIVE_PATH:-build/ios/Span.xcarchive}"
EXPORT_PATH="${EXPORT_PATH:-build/ios/export}"

xcodebuild \
  -project apps/ios/Span.xcodeproj \
  -scheme Span \
  -configuration Release \
  -destination 'generic/platform=iOS' \
  -archivePath "$ARCHIVE_PATH" \
  -allowProvisioningUpdates \
  archive

xcodebuild \
  -exportArchive \
  -archivePath "$ARCHIVE_PATH" \
  -exportOptionsPlist apps/ios/ExportOptions.plist \
  -exportPath "$EXPORT_PATH" \
  -allowProvisioningUpdates

echo "IPA exported to: $EXPORT_PATH"
