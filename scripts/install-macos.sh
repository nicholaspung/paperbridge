#!/bin/bash
# Installs the newest macOS build of PaperBridge from GitHub Actions into
# /Applications. Files fetched by the GitHub CLI are not quarantined, so
# Gatekeeper does not block the app the way it does after a browser download.
#
# Needs the GitHub CLI, signed in once:  brew install gh && gh auth login
# Usage:  bash install-macos.sh [run-id]
set -euo pipefail

REPO=nicholaspung/paperbridge
APP=/Applications/PaperBridge.app

run=${1:-$(gh run list -R "$REPO" -w build.yml --status success -L 30 \
  --json databaseId,event -q '[.[] | select(.event == "workflow_dispatch")][0].databaseId')}
if [[ -z "$run" || "$run" == "null" ]]; then
  echo "No successful macOS build found. Start one under Actions → build → Run workflow." >&2
  exit 1
fi
echo "Installing from run $run"

tmp=$(mktemp -d)
trap 'hdiutil detach "$tmp/mnt" -quiet 2>/dev/null || true; rm -rf "$tmp"' EXIT
gh run download "$run" -R "$REPO" -n PaperBridge-macos -D "$tmp/artifact"
dmg=$(find "$tmp/artifact" -name '*.dmg' | head -1)
[[ -n "$dmg" ]] || { echo "The build has no .dmg" >&2; exit 1; }

mkdir "$tmp/mnt"
hdiutil attach -nobrowse -readonly -mountpoint "$tmp/mnt" "$dmg" -quiet
if pgrep -xq PaperBridge; then
  echo "Quitting the running PaperBridge"
  osascript -e 'quit app "PaperBridge"' || true
  sleep 2
fi
rm -rf "$APP"
cp -R "$tmp/mnt/PaperBridge.app" "$APP"
# Belt and braces: the copy should carry no quarantine flag anyway.
xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true

probe=$(find "$tmp/artifact" -name pb-probe -type f | head -1)
if [[ -n "$probe" ]]; then
  mkdir -p "$HOME/bin"
  install -m 755 "$probe" "$HOME/bin/pb-probe"
  echo "pb-probe installed to ~/bin/pb-probe"
fi
echo "Installed $APP"
open "$APP"
