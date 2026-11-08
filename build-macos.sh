#!/bin/sh
# Builds KMSE for macOS into dist/ beside this file:
#   dist/KMSE.app                     the program (Universal 2: Apple silicon and Intel)
#   dist/KMSE-<version>-macOS.dmg     the app with a link to Applications, to drag across
#   dist/KMSE-<version>-macOS.zip     the app alone
#
#   sh build-macos.sh                 tests, then both architectures
#   sh build-macos.sh --arm64-only    Apple silicon only (no Intel half)
#   sh build-macos.sh --no-tests      skips the tests (the release build is the same)
#
# Needs Rust (rustup, stable) with the aarch64-apple-darwin target, and x86_64-apple-darwin
# for the Intel half (`rustup target add x86_64-apple-darwin`); Xcode's command line tools
# (clang, lipo, codesign, iconutil, sips, hdiutil, plutil, ditto). ffmpeg and KotOR installed
# are optional: with both, the timeline's Yavin Station is drawn from the game's movie
# (build.rs); without them it shows its name.
#
# Signing: with KMSE_SIGN_IDENTITY set to a "Developer ID Application: ..." identity in the
# keychain, the app is signed with it, the hardened runtime and a time stamp; otherwise it is
# signed ad hoc ("-"), which runs on the Mac that built it and, once the player allows it,
# on others. With KMSE_NOTARY_PROFILE also set (a profile stored with
# `xcrun notarytool store-credentials`), the DMG is sent to Apple's notary service and
# the ticket stapled. Nothing is uploaded or published otherwise.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
cd "$here"

universal=1
tests=1
for arg in "$@"; do
    case "$arg" in
        --arm64-only) universal=0 ;;
        --no-tests) tests=0 ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

fail() { echo "build-macos.sh: $*" >&2; exit 1; }

[ "$(uname)" = "Darwin" ] || fail "this script builds the macOS app; on Linux use build.sh, on Windows build.ps1"
for tool in cargo rustup lipo codesign iconutil sips hdiutil plutil ditto /usr/libexec/PlistBuddy; do
    command -v "$tool" >/dev/null 2>&1 || fail "$tool was not found"
done
targets="aarch64-apple-darwin"
[ "$universal" = 1 ] && targets="$targets x86_64-apple-darwin"
installed=$(rustup target list --installed)
for t in $targets; do
    echo "$installed" | grep -qx "$t" || fail "the Rust target $t is not installed: rustup target add $t"
done

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
[ -n "$version" ] || fail "no version in Cargo.toml"
name="KMSE"
bundle_id="com.raymangt.kotor-modern-gamesave-editor"
bin="kotor-modern-gamesave-editor"

# The program carries no path of this Mac: the sources' folders become fixed names in its
# panic messages (the only place a path is kept). Same sources, same program.
cargo_home=${CARGO_HOME:-$HOME/.cargo}
# CARGO_ENCODED_RUSTFLAGS, its flags apart by the 0x1f byte: RUSTFLAGS splits at spaces,
# and a checkout's folder may have them.
us=$(printf '\037')
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$here=/kmse$us--remap-path-prefix=$cargo_home=/cargo$us--remap-path-prefix=$HOME=/home"
unset RUSTFLAGS
# Apple silicon needs macOS 11; the Intel half is built for the same, so the app has one
# minimum. Set before the build: it is part of each binary's header.
export MACOSX_DEPLOYMENT_TARGET=11.0

if [ "$tests" = 1 ]; then
    echo "== tests"
    # in their own profile and folder, so they never change the release build (as build.ps1)
    CARGO_TARGET_DIR="$here/build/target-tests" cargo test --profile tests
fi

for t in $targets; do
    echo "== release build, $t"
    cargo build --release --locked --target "$t"
done

dist="$here/dist"
app="$dist/$name.app"
mkdir -p "$dist"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

echo "== bundle"
if [ "$universal" = 1 ]; then
    lipo -create \
        "$here/build/target/aarch64-apple-darwin/release/$bin" \
        "$here/build/target/x86_64-apple-darwin/release/$bin" \
        -output "$app/Contents/MacOS/$name"
else
    cp "$here/build/target/aarch64-apple-darwin/release/$bin" "$app/Contents/MacOS/$name"
fi
chmod 755 "$app/Contents/MacOS/$name"

# The icon, from the 1024 px master (assets/icon/icon.png), at every size macOS asks for.
iconset="$here/build/AppIcon.iconset"
rm -rf "$iconset"
mkdir -p "$iconset"
master="$here/assets/icon/icon.png"
for s in 16 32 128 256 512; do
    sips -z "$s" "$s" "$master" --out "$iconset/icon_${s}x${s}.png" >/dev/null
    d=$((s * 2))
    sips -z "$d" "$d" "$master" --out "$iconset/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/AppIcon.icns"
rm -rf "$iconset"

printf 'APPL????' > "$app/Contents/PkgInfo"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key><string>en</string>
    <key>CFBundleName</key><string>$name</string>
    <key>CFBundleDisplayName</key><string>$name</string>
    <key>CFBundleIdentifier</key><string>$bundle_id</string>
    <key>CFBundleExecutable</key><string>$name</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
    <key>NSHumanReadableCopyright</key><string>KOTOR Modern Gamesave Editor. GPL-3.0-or-later.</string>
</dict>
</plist>
PLIST
plutil -lint "$app/Contents/Info.plist" >/dev/null

echo "== signing"
identity=${KMSE_SIGN_IDENTITY:-}
if [ -n "$identity" ]; then
    codesign --force --options runtime --timestamp --sign "$identity" "$app"
else
    echo "no KMSE_SIGN_IDENTITY: signed ad hoc (not for distribution without notarization)"
    codesign --force --sign - "$app"
fi

echo "== checks"
codesign --verify --strict --verbose=1 "$app"
archs=$(lipo -archs "$app/Contents/MacOS/$name")
echo "architectures: $archs"
echo "$archs" | grep -q arm64 || fail "the program has no arm64 code"
if [ "$universal" = 1 ]; then
    echo "$archs" | grep -q x86_64 || fail "the program has no x86_64 code"
fi
# nothing outside the system's own libraries is loaded, in either half (the lines naming the
# program and its architecture end in a colon and are skipped; the path may have spaces)
otool -arch all -L "$app/Contents/MacOS/$name" | grep -v ':$' | awk '{print $1}' | while read -r lib; do
    case "$lib" in
        /usr/lib/*|/System/Library/*) ;;
        *) fail "the program loads $lib, which a Mac without it lacks" ;;
    esac
done
# no path of this Mac in the program
if strings -a "$app/Contents/MacOS/$name" | grep -F -q "$HOME"; then
    fail "the program holds a path under $HOME"
fi
[ -f "$app/Contents/Resources/AppIcon.icns" ] || fail "no icon"
if command -v spctl >/dev/null 2>&1; then
    spctl --assess --type execute "$app" 2>/dev/null && echo "Gatekeeper: accepted" || echo "Gatekeeper: not accepted (expected unless signed with a Developer ID and notarized)"
fi

echo "== packages"
dmg="$dist/$name-$version-macOS.dmg"
zip="$dist/$name-$version-macOS.zip"
rm -f "$dmg" "$zip"
stage="$here/build/dmg"
rm -rf "$stage"
mkdir -p "$stage"
ditto "$app" "$stage/$name.app"
ln -s /Applications "$stage/Applications"
hdiutil create -quiet -volname "$name $version" -srcfolder "$stage" -fs HFS+ -format UDZO -ov "$dmg"
rm -rf "$stage"
ditto -c -k --sequesterRsrc --keepParent "$app" "$zip"
if [ -n "$identity" ]; then
    codesign --force --timestamp --sign "$identity" "$dmg"
    if [ -n "${KMSE_NOTARY_PROFILE:-}" ]; then
        xcrun notarytool submit "$dmg" --keychain-profile "$KMSE_NOTARY_PROFILE" --wait
        xcrun stapler staple "$dmg"
        xcrun stapler staple "$app"
        rm -f "$zip"
        ditto -c -k --sequesterRsrc --keepParent "$app" "$zip"
    fi
fi

echo
for f in "$app/Contents/MacOS/$name" "$dmg" "$zip"; do
    printf '%s  %s bytes  sha256 %s\n' "$f" "$(stat -f %z "$f")" "$(shasum -a 256 "$f" | cut -d' ' -f1)"
done
