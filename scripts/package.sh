#!/bin/sh
# Assembles a release package from the built launcher and the components.
#
#   scripts/package.sh linux     dist/pkg/NintendoArcadeLauncher-<version>-linux-x86_64.tar.gz
#   scripts/package.sh windows   dist/pkg/NintendoArcadeLauncher-<version>-windows-x86_64.zip
#
# Built pieces (scripts/build-launcher.sh, the release workflow):
#   linux    dist/linux/nintendo-launcher, dist/linux/lib/libSDL2-2.0.so.0
#   windows  target/x86_64-pc-windows-gnu/release/nintendo-launcher.exe, dist/sdl2/SDL2.dll
#            (sdk/packaging/build-launcher.sh --root . windows nintendo-launcher)
# The components are built or fetched once into dist/components with
# scripts/build-components.sh (Dolphin for Linux from its tag, with the
# libraries it needs; Dolphin for Windows, GE-Proton and winetricks by
# SHA-256; windowsarcadeloader from source with patches/wal, both its
# launchers; the CRT effect and the art), then copied into each
# package. No game file is ever packaged.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd -P)
os=${1:-linux}
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)
die() { echo "package: $*" >&2; exit 1; }
comps=$root/dist/components

case $os in
linux) name=NintendoArcadeLauncher-$version-linux-x86_64 ;;
windows) name=NintendoArcadeLauncher-$version-windows-x86_64 ;;
*) die "unknown target '$os' (linux or windows)" ;;
esac

out=$root/dist/pkg/$name
rm -rf "$out"
mkdir -p "$out/components/wal" "$out/licenses"

copy_wal() { # the launcher's own program of windowsarcadeloader
	cp "$comps/wal/$1" "$out/components/wal/"
	cp -r "$comps/wal/payloads" "$comps/wal/systemprofiles" "$out/components/wal/"
	cp "$comps/wal/LICENSE" "$out/licenses/windowsarcadeloader-LICENSE.txt"
	cp -r "$comps/art" "$comps/shaders" "$out/components/"
}

if [ "$os" = linux ]; then
	bin=$root/dist/linux
	[ -f "$bin/nintendo-launcher" ] || die "missing $bin/nintendo-launcher - build it first"
	[ -f "$bin/lib/libSDL2-2.0.so.0" ] || die "missing $bin/lib/libSDL2-2.0.so.0"
	"$root/scripts/build-components.sh" "$comps" dolphin wal runner winetricks shaders assets
	install -m 755 "$bin/nintendo-launcher" "$out/nintendo-launcher"
	mkdir -p "$out/lib"
	cp "$bin"/lib/* "$out/lib/"
	[ -f "$bin/SDL2-copyright.txt" ] && cp "$bin/SDL2-copyright.txt" "$out/licenses/"
	copy_wal arcade-launcher
	cp -r "$comps/runner" "$out/components/runner"
	install -m 755 "$comps/winetricks" "$out/components/winetricks"
	cp -r "$comps/dolphin" "$out/components/dolphin"
	rm -f "$out/components/dolphin/.stamp"
	cp "$comps/dolphin/COPYING" "$out/licenses/Dolphin-COPYING.txt"
else
	exe=${CARGO_TARGET_DIR:-$root/target}/x86_64-pc-windows-gnu/release/nintendo-launcher.exe
	[ -f "$exe" ] || die "missing $exe - run sdk/packaging/build-launcher.sh --root . windows nintendo-launcher"
	[ -f "$root/dist/sdl2/SDL2.dll" ] || die "missing dist/sdl2/SDL2.dll"
	"$root/scripts/build-components.sh" "$comps" dolphin-win wal shaders assets
	cp "$exe" "$root/dist/sdl2/SDL2.dll" "$out/"
	cp "$(ls -d "$root"/dist/sdl2/SDL2-*/ | head -1)LICENSE.txt" "$out/licenses/SDL2-LICENSE.txt"
	copy_wal arcade-launcher.exe
	cp -r "$comps/dolphin-win" "$out/components/dolphin-win"
	cp "$comps/dolphin-win/COPYING" "$out/licenses/Dolphin-COPYING.txt"
fi

cp "$root/packaging/README-player.txt" "$out/README.txt"
cp "$root/packaging/SOURCES.txt" "$out/licenses/SOURCES.txt"
cp "$root/LICENSE" "$out/licenses/LICENSE-GPL-3.0.txt"
cp "$root/THIRD_PARTY.md" "$out/licenses/"
cp "$root/assets/fonts/OFL.txt" "$out/licenses/Barlow-OFL.txt"

cd "$root/dist/pkg"
if [ "$os" = linux ]; then
	tar -czf "$name.tar.gz" "$name"
	echo "package: dist/pkg/$name.tar.gz"
else
	rm -f "$name.zip"
	zip -qr "$name.zip" "$name"
	echo "package: dist/pkg/$name.zip"
fi
