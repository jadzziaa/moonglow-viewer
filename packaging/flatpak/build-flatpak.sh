#!/bin/sh
# Builds target/dist/MoonglowViewer-<version>-<arch>.flatpak, a single-file
# bundle (install it with `flatpak install --user MoonglowViewer-*.flatpak`).
#
# Needs flatpak-builder and, from Flathub, the Freedesktop 25.08 SDK with
# its rust-stable extension (installed for the user when missing). The
# crates, the Moonglow Toolset's from its git repository included, are
# vendored first (`cargo vendor`, with the network and your git
# credentials), so the sandboxed build runs offline.
set -eu
cd "$(dirname "$0")/../.."
ID=io.github.moonglow_toolset.MoonglowViewer
ARCH=$(flatpak --default-arch)
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
OUT=target/dist
VENDOR=packaging/flatpak/vendor
# Where the build finds the sources inside the sandbox.
BUILD=/run/build/moonglow-viewer

rm -rf "$VENDOR"
cargo vendor --locked --versioned-dirs "$VENDOR" |
    sed "s#^directory = .*#directory = \"$BUILD/$VENDOR\"#" > packaging/flatpak/vendor-config.toml
# License notices need cargo metadata, which needs the network: made here.
python3 packaging/third_party_licenses.py --output packaging/flatpak/THIRD-PARTY-LICENSES.txt
flatpak-builder --user --install-deps-from=flathub --force-clean \
    --state-dir=target/flatpak-builder --repo=target/flatpak-repo \
    target/flatpak-build packaging/flatpak/$ID.yml
mkdir -p "$OUT"
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
    target/flatpak-repo "$OUT/MoonglowViewer-$VERSION-$ARCH.flatpak" $ID
echo "$OUT/MoonglowViewer-$VERSION-$ARCH.flatpak"
