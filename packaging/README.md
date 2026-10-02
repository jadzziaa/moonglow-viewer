# Packaging

Moonglow Viewer ships as an AppImage and a Flatpak for Linux, an installer
for Windows, and a universal app in a disk image for macOS. Each package
holds both programs: `moonglow-viewer` (the window) and `mgv` (the command
line), with the license, the third-party license notices
(`third_party_licenses.py`) and the user manual.

| Package | Script | Where it runs | Output (`target/dist/`) |
| --- | --- | --- | --- |
| AppImage | `linux/build-appimage.sh` | Linux, with `appimagetool` | `MoonglowViewer-<version>-<arch>.AppImage` |
| Flatpak | `flatpak/build-flatpak.sh` | Linux, with `flatpak-builder` | `MoonglowViewer-<version>-<arch>.flatpak` |
| Windows installer | `windows/build-installer.ps1` | Windows, with Inno Setup 6 | `MoonglowViewer-<version>-windows-x64-setup.exe` |
| macOS app | `macos/build-app.sh` | macOS, both Rust targets | `MoonglowViewer-<version>-macos.dmg` |

All build with the `dist` profile (thin LTO, symbols kept for crash
reports). The version is the workspace's in `Cargo.toml`.

The release workflow (`.github/workflows/release.yml`) builds the AppImage,
the installer and the disk image on a `v*` tag and drafts a release with
them. The Flatpak is built locally.

## The Moonglow Toolset crates

The viewer's toolset crates come from github.com/jadzziaa/moonglow-toolset,
at a release tag (`Cargo.toml`; the commit in `Cargo.lock`). While that
repository is private:

- CI and the release workflow read it with a token: add a repository
  secret `TOOLSET_TOKEN` (a fine-grained token with read access to its
  contents).
- The Flatpak build vendors every crate first (`cargo vendor`, with your
  git credentials), then builds offline in the sandbox.

## Names

The app ID `io.github.moonglow_toolset.MoonglowViewer` (macOS:
`io.github.moonglow-toolset.MoonglowViewer`, bundle IDs take no
underscores) names the desktop entry, the icon, the AppStream component and
the window (`APP_ID` in `apps/moonglow-viewer/src/main.rs`); change them
together. `.mdl` files are offered as `model/x-nwn-mdl` (Linux), an
optional association (Windows) and a document type (macOS); other programs
use `.mdl` too, so none of them claims the extension by default.

## The icon

`icons/moonglow-viewer.svg` is the source; `icons/render.sh` renders the
PNGs, the `.ico` and the `.icns` (rsvg-convert and Pillow), which are
committed.

## Signing

The Windows installer is unsigned and the macOS app is signed ad hoc. To
sign the macOS app with a Developer ID, set `CODESIGN_IDENTITY`, then
notarize the disk image (`xcrun notarytool submit --wait`, `xcrun stapler
staple`).
