#!/bin/sh
# Builds KMSE for Linux as an AppImage, into dist/ beside this file:
#   dist/KMSE-Linux-<version>-x86_64.AppImage    the program, one file to download and run
#
#   sh build-linux.sh               tests, then the release build, then the AppImage
#   sh build-linux.sh --no-tests    skips the tests (the release build is the same)
#   sh build-linux.sh --pack-only   packs the program already in build/target/release
#
# Packed the way KMRP packs its Linux installer (KMRP's tools/build_linux_appimage.sh): an
# AppDir with the program, a launcher (AppRun), a desktop entry and the icon, made into one
# file by appimagetool 1.9.1 from the AppImage project's GitHub release, downloaded once into
# build/tools and checked against the SHA-256 GitHub publishes for it. appimagetool fetches
# the AppImage runtime it puts in front of the program from GitHub as it packs.
#
# Needs Rust (rustup, stable), curl, sha256sum, and the window libraries' development
# packages (CONTRIBUTING.md, "Prerequisites"). readelf and objdump (binutils), where present,
# check what the program loads. The AppImage carries the program alone: the libraries it
# loads (fontconfig; X11, xcb, Xcursor, Xi and xkbcommon, or Wayland and xkbcommon) are a
# Linux desktop's own and are taken from the system, as AppImages do. It runs on Linux with
# the glibc of the machine that built it or newer: CI builds it on Ubuntu 22.04 (glibc 2.35).
set -eu

here=$(cd "$(dirname "$0")" && pwd)
cd "$here"

tests=1
build=1
for arg in "$@"; do
    case "$arg" in
        --no-tests) tests=0 ;;
        --pack-only) tests=0; build=0 ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

fail() { echo "build-linux.sh: $*" >&2; exit 1; }

[ "$(uname)" = "Linux" ] || fail "this script builds the Linux AppImage; on macOS use build-macos.sh, on Windows build.ps1"
[ "$(uname -m)" = "x86_64" ] || fail "the AppImage is built for x86_64; this machine is $(uname -m)"
for tool in curl sha256sum; do
    command -v "$tool" >/dev/null 2>&1 || fail "$tool was not found"
done
[ "$build" = 0 ] || command -v cargo >/dev/null 2>&1 || fail "cargo was not found"

# (without the carriage return a checkout made on Windows ends its lines with, as under WSL)
version=$(tr -d '\r' < Cargo.toml | sed -n 's/^version = "\(.*\)"/\1/p' | head -n 1)
[ -n "$version" ] || fail "no version in Cargo.toml"
bin="kotor-modern-gamesave-editor"

# The program carries no path of this machine: the sources' folders become fixed names in
# its panic messages (the only place a path is kept), as in build-macos.sh. Same sources,
# same program. CARGO_ENCODED_RUSTFLAGS, its flags apart by the 0x1f byte: RUSTFLAGS splits
# at spaces, and a checkout's folder may have them.
cargo_home=${CARGO_HOME:-$HOME/.cargo}
us=$(printf '\037')
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$here=/kmse$us--remap-path-prefix=$cargo_home=/cargo$us--remap-path-prefix=$HOME=/home"
unset RUSTFLAGS

if [ "$tests" = 1 ]; then
    echo "== tests"
    # in their own profile and folder, so they never change the release build (as build.ps1)
    CARGO_TARGET_DIR="$here/build/target-tests" cargo test --profile tests
fi

if [ "$build" = 1 ]; then
    echo "== release build"
    cargo build --release --locked
fi
program="$here/build/target/release/$bin"
[ -f "$program" ] || fail "no program at $program"

echo "== checks"
# nothing outside glibc, libgcc and fontconfig is linked; the window's libraries are opened
# at run time from the system
if command -v readelf >/dev/null 2>&1; then
    readelf -d "$program" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p' | while read -r lib; do
        case "$lib" in
            libc.so.*|libm.so.*|libgcc_s.so.*|libpthread.so.*|libdl.so.*|librt.so.*|ld-linux-*|libfontconfig.so.1) ;;
            *) fail "the program links $lib, which the AppImage does not carry" ;;
        esac
    done
else
    echo "no readelf: the linked libraries are not checked"
fi
if command -v objdump >/dev/null 2>&1; then
    glibc=$(objdump -T "$program" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/GLIBC_//' | sort -t. -k1,1n -k2,2n -k3,3n | tail -n 1)
    echo "needs glibc $glibc or newer"
fi
# no path of this machine in the program
if strings -a "$program" 2>/dev/null | grep -F -q "$HOME"; then
    fail "the program holds a path under $HOME"
fi

echo "== appimagetool"
tool="$here/build/tools/appimagetool-x86_64.AppImage"
tool_url="https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"
tool_sha256="ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"
if [ ! -f "$tool" ] || ! echo "$tool_sha256  $tool" | sha256sum -c --status -; then
    mkdir -p "$(dirname "$tool")"
    curl -fsSL -o "$tool" "$tool_url"
    echo "$tool_sha256  $tool" | sha256sum -c - || { rm -f "$tool"; fail "appimagetool's SHA-256 is not the published one"; }
fi
chmod +x "$tool"

echo "== AppDir"
# The program, the launcher, the desktop entry and the window's icon (assets/save-editor-icon.png,
# 512 x 512), at the root where appimagetool looks for them and in the usual places.
appdir="$here/build/AppDir"
rm -rf "$appdir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/applications" "$appdir/usr/share/icons/hicolor/512x512/apps"
cp "$program" "$appdir/usr/bin/$bin"
chmod 755 "$appdir/usr/bin/$bin"
cp "$here/assets/save-editor-icon.png" "$appdir/$bin.png"
cp "$here/assets/save-editor-icon.png" "$appdir/usr/share/icons/hicolor/512x512/apps/$bin.png"
cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/kotor-modern-gamesave-editor" "$@"
EOF
chmod 755 "$appdir/AppRun"
cat > "$appdir/$bin.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=KOTOR Modern Gamesave Editor
GenericName=Save Editor
Comment=Opens Star Wars: Knights of the Old Republic and KotOR II saves and changes them safely
Exec=$bin
Icon=$bin
Categories=Game;
Terminal=false
EOF
cp "$appdir/$bin.desktop" "$appdir/usr/share/applications/$bin.desktop"

echo "== AppImage"
dist="$here/dist"
target="$dist/KMSE-Linux-$version-x86_64.AppImage"
mkdir -p "$dist"
rm -f "$target"
# The tool runs from its own extraction, so it needs no FUSE (WSL and CI have none).
ARCH=x86_64 VERSION="$version" APPIMAGE_EXTRACT_AND_RUN=1 "$tool" --no-appstream "$appdir" "$target"
chmod 755 "$target"

echo
printf '%s  %s bytes  sha256 %s\n' "$target" "$(stat -c %s "$target")" "$(sha256sum "$target" | cut -d' ' -f1)"
