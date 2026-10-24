#!/bin/sh
# Builds the KOTOR Modern Gamesave Editor for Linux or macOS into dist/ beside this file:
#   Linux:  dist/kotor-modern-gamesave-editor
#   macOS:  dist/KOTOR Modern Gamesave Editor.app
#
#   sh build.sh
#
# Needs Rust (rustup, stable); everything else is in this folder, so it builds on its own,
# outside KMRP (build.rs says where KMRP's fonts are looked for). On Linux the window also
# needs the usual X11/Wayland and fontconfig development packages.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
cd "$here"
cargo test --release
cargo build --release
mkdir -p "$here/dist"
bin="$here/build/target/release/kotor-modern-gamesave-editor"
if [ "$(uname)" = "Darwin" ]; then
    app="$here/dist/KOTOR Modern Gamesave Editor.app"
    rm -rf "$app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$bin" "$app/Contents/MacOS/kotor-modern-gamesave-editor"
    cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>KOTOR Modern Gamesave Editor</string>
<key>CFBundleDisplayName</key><string>KOTOR Modern Gamesave Editor</string>
<key>CFBundleIdentifier</key><string>com.raymangt.kotor-modern-gamesave-editor</string>
<key>CFBundleExecutable</key><string>kotor-modern-gamesave-editor</string>
<key>CFBundleShortVersionString</key><string>1.0</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
    echo "$app"
else
    cp "$bin" "$here/dist/kotor-modern-gamesave-editor"
    echo "$here/dist/kotor-modern-gamesave-editor"
fi
