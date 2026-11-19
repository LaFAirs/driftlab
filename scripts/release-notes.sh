#!/usr/bin/env bash
# Usage: release-notes.sh vX.Y.Z owner/repo  → prints release notes (CHANGELOG section + download table).
set -euo pipefail
TAG="$1"
REPO="$2"
VER="${TAG#v}"
SECTION="$(awk -v ver="$VER" '
  $0 ~ "^## \\[" ver "\\]" { grab = 1; next }
  grab && /^## \[/ { exit }
  grab { print }
' CHANGELOG.md)"
if [ -z "$SECTION" ]; then SECTION="See CHANGELOG.md for details."; fi
BASE="https://github.com/${REPO}/releases/download/${TAG}"
cat <<EOF
${SECTION}

## Downloads

| Platform | File |
|---|---|
| Windows 10/11 x64 (installer) | [DriftLab-${VER}-windows-x64-setup.exe](${BASE}/DriftLab-${VER}-windows-x64-setup.exe) |
| Windows 10/11 x64 (portable) | [DriftLab-${VER}-windows-x64-portable.zip](${BASE}/DriftLab-${VER}-windows-x64-portable.zip) |
| Linux x64 (AppImage) | [DriftLab-${VER}-linux-x86_64.AppImage](${BASE}/DriftLab-${VER}-linux-x86_64.AppImage) |
| Linux x64 (Debian/Ubuntu) | [DriftLab-${VER}-linux-amd64.deb](${BASE}/DriftLab-${VER}-linux-amd64.deb) |
| macOS (universal) | [DriftLab-${VER}-macos-universal.dmg](${BASE}/DriftLab-${VER}-macos-universal.dmg) |
| Checksums | [SHA256SUMS.txt](${BASE}/SHA256SUMS.txt) |

> The binaries are not code-signed unless stated otherwise. Windows SmartScreen / macOS Gatekeeper may warn on first launch – see the README FAQ.
> Hardware calibration writes to your controller. Read the safety notes in the README first.
EOF
