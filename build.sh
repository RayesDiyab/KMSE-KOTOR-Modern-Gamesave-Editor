#!/bin/sh
# Builds the KOTOR Modern Gamesave Editor for Linux into dist/ beside this file:
#   dist/kotor-modern-gamesave-editor
# On macOS it runs build-macos.sh instead (dist/KMSE.app, a DMG and a ZIP; see there).
#
#   sh build.sh
#
# Needs Rust (rustup, stable); everything else is in this folder, so it builds on its own,
# outside KMRP (build.rs says where KMRP's fonts are looked for). On Linux the window also
# needs the usual X11/Wayland and fontconfig development packages.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
if [ "$(uname)" = "Darwin" ]; then
    exec sh "$here/build-macos.sh" "$@"
fi
cd "$here"
cargo test --release
cargo build --release
mkdir -p "$here/dist"
cp "$here/build/target/release/kotor-modern-gamesave-editor" "$here/dist/kotor-modern-gamesave-editor"
echo "$here/dist/kotor-modern-gamesave-editor"
