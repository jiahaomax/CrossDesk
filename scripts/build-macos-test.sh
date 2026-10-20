#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
test "$(uname -s)" = Darwin || { echo 'macOS build host required'; exit 1; }
root="$PWD"
arch=$(uname -m)
case "$arch" in arm64|x86_64) ;; *) echo 'Unsupported architecture'; exit 1;; esac
name="CrossDesk-Test-0.3.0-rc17-Mac-$arch"
out="$root/dist/$name"
test ! -e "$out" && test ! -e "$out.zip" || { echo 'Output already exists'; exit 1; }
export CARGO_TARGET_DIR="$root/target/test-macos-build"
cargo test --workspace --exclude crossdesk-desktop --locked --features crossdesk-daemon/test-profile
cargo build -p crossdesk-daemon --release --locked --features test-profile
daemon="$CARGO_TARGET_DIR/release/crossdesk-daemon"
"$daemon" --profile-info | python3 -c 'import json,sys; p=json.load(sys.stdin); assert p["test"] and p["bundle_id"]=="dev.crossdesk.desktop.test" and p["port"]==24972'
triple=$(rustc -vV | sed -n 's/^host: //p')
mkdir -p apps/desktop/src-tauri/binaries
cp "$daemon" "apps/desktop/src-tauri/binaries/crossdesk-test-daemon-$triple"
npm ci --prefix apps/desktop
cd apps/desktop
npm run tauri -- build --bundles app --config src-tauri/tauri.test.conf.json --features test-profile
cd "$root"
app="$CARGO_TARGET_DIR/release/bundle/macos/CrossDesk Test.app"
test -d "$app"
test "$(/usr/libexec/PlistBuddy -c 'Print CFBundleIdentifier' "$app/Contents/Info.plist")" = dev.crossdesk.desktop.test
"$app/Contents/MacOS/crossdesk-test-daemon" --profile-info | python3 -c 'import json,sys; assert json.load(sys.stdin)["test"]'
/usr/bin/codesign --verify --deep --strict "$app"
if [ "${GITHUB_ACTIONS:-}" = true ]; then
  python3 scripts/check-macos-test-edition.py "$app/Contents/MacOS/crossdesk-test-daemon"
fi
mkdir -p "$out"
ditto "$app" "$out/CrossDesk Test.app"
cp scripts/Cleanup-CrossDesk-Test.command "$out/"
chmod +x "$out/Cleanup-CrossDesk-Test.command"
cp docs/TEST_EDITION.md "$out/使用说明.md"
printf 'CrossDesk Test 0.3.0 rc17\nmacOS %s\nAd-hoc signed; not notarized; native input acceptance pending\n' "$arch" > "$out/VERSION.txt"
ditto -c -k --sequesterRsrc --keepParent "$out" "$out.zip"
echo "Ready: $out.zip"
