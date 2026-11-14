#!/bin/sh
# Builds the launcher's components from pinned sources into a components folder:
#   dolphin/      Dolphin (GPL-2.0-or-later) for Linux, built here from its tag: the headless-
#                 capable dolphin-emu-nogui, its Sys/ data beside it and the libraries it needs
#                 that a desktop may not have in lib/ (needs cmake, ninja, a C++23 compiler and
#                 Dolphin's build dependencies); not rebuilt while the tag is unchanged
#   dolphin-win/  Dolphin for Windows, the project's own release, checked by SHA-256
#   wal/          windowsarcadeloader (GPL-3.0) with patches/wal applied: arcade-launcher (Linux),
#                 arcade-launcher.exe (its Windows build, linked with LLD), payloads/,
#                 systemprofiles/  (needs Rust with the i686/x86_64-pc-windows-gnu targets, MinGW,
#                 LLD and mono's mcs); not rebuilt while its commit and patches are unchanged
#   runner/       GE-Proton 11-12, checked by SHA-256
#   winetricks    pinned, checked by SHA-256 (windowsarcadeloader installs the game's runtimes)
#   shaders/      the launcher's own CRT effect
#   art/ fonts/   the window's original illustrations and the Barlow typefaces (OFL)
# usage: scripts/build-components.sh <components dir> [dolphin|dolphin-win|wal|runner|winetricks|shaders|assets ...]
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
out=$(mkdir -p "$1" && cd "$1" && pwd); shift
parts=${*:-dolphin wal runner winetricks shaders assets}
work="$out/.build"; mkdir -p "$work"

DOLPHIN_REPO=https://github.com/dolphin-emu/dolphin
DOLPHIN_TAG=2609b
DOLPHIN_COMMIT=3a5b3743b18da3e8f04cbc44f20dc50fbd3a9ef3
DOLPHIN_WIN_URL=https://dl.dolphin-emu.org/releases/2609b/dolphin-2609b-x64.7z
DOLPHIN_WIN_SHA256=926eb7594d885b4fba8fd9807058d90618efab4b3d75d1ef297022ba14925133
WAL_REPO=https://github.com/aderumier/windowsarcadeloader
WAL_COMMIT=4f42c6d86ddaaa9d2e4e8c21e0046bb3027b111d
RUNNER=GE-Proton11-12-x86_64
RUNNER_URL=https://github.com/GloriousEggroll/proton-ge-custom/releases/download/GE-Proton11-12/$RUNNER.tar.gz
RUNNER_SHA256=0595159e417c3dbbfb7cc3ae0d3ed15a617cc607aa23ef756ea79029d7af5d70
WINETRICKS_URL=https://raw.githubusercontent.com/Winetricks/winetricks/20260125/src/winetricks
WINETRICKS_SHA256=431f82fc74000e6c864409f1d8fb495d696c03928808e3e8acffc45179312a7b

checkout() { # repo commit dir
  [ -d "$3/.git" ] || git clone -q "$1" "$3"
  git -C "$3" fetch -q origin "$2" 2>/dev/null || true
  git -C "$3" checkout -q --force "$2"
  git -C "$3" clean -qfdx
}

# Libraries every Linux desktop has, or that must come from its own driver
# stack: never copied beside Dolphin.
system_lib() {
  case $1 in
  */*|linux-vdso*|ld-linux*|libc.so*|libm.so*|libdl.so*|librt.so*|libpthread.so*|libresolv.so*|\
  libstdc++.so*|libgcc_s.so*|libGL*|libEGL*|libGLX*|libGLdispatch*|libOpenGL*|libvulkan*|libdrm*|libgbm*|\
  libX*|libxcb*|libxkb*|libwayland*|libasound*|libpulse*|libudev*|libsystemd*|libdbus*|libz.so*|\
  libcap*|liblzma*|libzstd*|libgcrypt*|libgpg-error*|liblz4*|libmount*|libblkid*|libselinux*|libpcre*|\
  libffi*|libbsd*|libmd.so*|libXau*|libXdmcp*) return 0 ;;
  esac
  return 1
}

for part in $parts; do case $part in
dolphin)
  stamp="$DOLPHIN_COMMIT"
  if [ -f "$out/dolphin/.stamp" ] && [ "$(cat "$out/dolphin/.stamp")" = "$stamp" ]; then
    echo "dolphin: up to date ($DOLPHIN_TAG)"
    continue
  fi
  src="$work/dolphin"
  [ -d "$src/.git" ] || git clone -q --filter=blob:none "$DOLPHIN_REPO" "$src"
  git -C "$src" fetch -q origin "refs/tags/$DOLPHIN_TAG:refs/tags/$DOLPHIN_TAG" 2>/dev/null || true
  git -C "$src" checkout -q --force "$DOLPHIN_COMMIT"
  git -C "$src" submodule update -q --init --recursive --depth 1
  cmake -S "$src" -B "$work/dolphin-build" -G Ninja -DCMAKE_BUILD_TYPE=Release \
    -DENABLE_QT=OFF -DENABLE_NOGUI=ON -DENABLE_HEADLESS=OFF -DENABLE_TESTS=OFF -DENABLE_CLI_TOOL=OFF \
    -DUSE_DISCORD_PRESENCE=OFF -DENABLE_ANALYTICS=OFF -DENABLE_AUTOUPDATE=OFF \
    -DUSE_RETRO_ACHIEVEMENTS=OFF -DENABLE_LLVM=OFF -DUSE_MGBA=OFF -DUSE_UPNP=OFF \
    -DENCODE_FRAMEDUMPS=OFF -DUSE_SYSTEM_LIBS=AUTO -DDISTRIBUTOR=rea-teknoparrot-nintendo-series
  cmake --build "$work/dolphin-build" --target dolphin-nogui
  rm -rf "$out/dolphin"; mkdir -p "$out/dolphin/lib"
  install -m 755 "$work/dolphin-build/Binaries/dolphin-emu-nogui" "$out/dolphin/"
  cp -r "$src/Data/Sys" "$out/dolphin/Sys"
  cp "$src/COPYING" "$out/dolphin/COPYING"
  cp -r "$src/LICENSES" "$out/dolphin/LICENSES"
  # The libraries Dolphin needs that are not the system's, and theirs in turn: a
  # system library's own dependencies stay the system's.
  map="$work/dolphin-ldd"
  ldd "$out/dolphin/dolphin-emu-nogui" | awk '$2 == "=>" && $3 ~ /^\// {print $1, $3}' > "$map"
  needed() { readelf -d "$1" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p'; }
  queue=$(needed "$out/dolphin/dolphin-emu-nogui")
  while [ -n "$queue" ]; do
    next=""
    for name in $queue; do
      system_lib "$name" && continue
      [ -f "$out/dolphin/lib/$name" ] && continue
      path=$(awk -v n="$name" '$1 == n {print $2}' "$map")
      [ -n "$path" ] || { echo "dolphin: $name not found" >&2; exit 1; }
      cp -L "$path" "$out/dolphin/lib/$name"
      next="$next $(needed "$path")"
    done
    queue=$next
  done
  echo "$stamp" > "$out/dolphin/.stamp"
  ;;
dolphin-win)
  f="$work/dolphin-win.7z"
  [ -f "$f" ] || curl -fsSL -A "Mozilla/5.0" -o "$f" "$DOLPHIN_WIN_URL"
  echo "$DOLPHIN_WIN_SHA256  $f" | sha256sum -c -
  rm -rf "$out/dolphin-win" "$work/Dolphin-x64"
  (cd "$work" && 7z x -y -bd "$f" >/dev/null)
  mv "$work/Dolphin-x64" "$out/dolphin-win"
  # the release's self-updater has no use here
  rm -f "$out/dolphin-win/Updater.exe"
  ;;
wal)
  stamp="$WAL_COMMIT $(cat "$here"/patches/wal/*.patch | sha256sum | cut -c1-16)"
  if [ -f "$out/wal/.stamp" ] && [ "$(cat "$out/wal/.stamp")" = "$stamp" ]; then
    echo "wal: up to date ($stamp)"
    continue
  fi
  checkout "$WAL_REPO" "$WAL_COMMIT" "$work/windowsarcadeloader"
  for p in "$here"/patches/wal/*.patch; do git -C "$work/windowsarcadeloader" apply "$p"; done
  (cd "$work/windowsarcadeloader" && ./build.sh)
  # the Windows host build (patches/wal/0002), linked with LLD (GNU ld 2.47 mis-links Rust's
  # Windows import tables)
  (cd "$work/windowsarcadeloader" && \
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C link-arg=-fuse-ld=lld" \
    cargo build --release --target x86_64-pc-windows-gnu -p wal-winhost)
  mkdir -p "$out/wal"
  cp "$work/windowsarcadeloader/dist/arcade-launcher" "$out/wal/"
  cp "$work/windowsarcadeloader/target/x86_64-pc-windows-gnu/release/arcade-launcher.exe" "$out/wal/"
  rm -rf "$out/wal/payloads" "$out/wal/systemprofiles"
  cp -r "$work/windowsarcadeloader/dist/payloads" "$out/wal/payloads"
  cp -r "$work/windowsarcadeloader/systemprofiles" "$out/wal/systemprofiles"
  cp "$work/windowsarcadeloader/LICENSE" "$out/wal/LICENSE"
  echo "$stamp" > "$out/wal/.stamp"
  ;;
runner)
  f="$work/$RUNNER.tar.gz"
  [ -f "$f" ] || curl -fsSL -o "$f" "$RUNNER_URL"
  echo "$RUNNER_SHA256  $f" | sha256sum -c -
  tar -xzf "$f" -C "$work"
  rm -rf "$out/runner"; mv "$work/$RUNNER/files" "$out/runner"
  ;;
winetricks)
  f="$work/winetricks"
  [ -f "$f" ] || curl -fsSL -o "$f" "$WINETRICKS_URL"
  echo "$WINETRICKS_SHA256  $f" | sha256sum -c -
  install -m 755 "$f" "$out/winetricks"
  ;;
shaders)
  mkdir -p "$out/shaders"; cp "$here"/assets/shaders/*.fx "$out/shaders/"
  ;;
assets)
  rm -rf "$out/art" "$out/fonts"; cp -r "$here/assets/art" "$out/art"; cp -r "$here/assets/fonts" "$out/fonts"
  ;;
*) echo "unknown part $part" >&2; exit 2;;
esac; done
echo "components ready in $out"
