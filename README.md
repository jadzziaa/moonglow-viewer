# Moonglow Viewer

A model viewer for **Neverwinter Nights: Enhanced Edition**, a sibling of
[Moonglow Toolset](https://github.com/jadzziaa/moonglow-toolset), for
Linux, Windows and macOS. Written in Rust, with an
[egui](https://github.com/emilk/egui) interface and a [wgpu](https://wgpu.rs)
renderer.

- **Every visual file the game uses**: models (compiled and ASCII),
  walkmeshes, TGA, DDS and PLT textures with their TXI, blueprints
  (creatures, items, placeables, doors) and creatures by `appearance.2da`
  row, from the game's load order, haks, modules and loose folders.
- **The game's lighting**: the EE client's enhanced lighting (the toolset's
  renderer, matched to the client), with studio, area-preset and custom
  rigs; emitters running and `visualeffects.2da` effects attached to their
  hooks.
- **One-click decompile and compile**: an exact native decompiler (all
  25,597 compiled models of the game read back the same), nwnmdlcomp, and
  the game's own model compiler.
- **An ASCII editor with hot reload**: the view follows every edit and
  every change made by other programs; diagnostics by line; cursor and
  selection in step with the view; fast on the game's largest models.
- **Headless**: `mgv render`, `turntable`, `gallery` (thumbnails with an
  HTML index and a JSON manifest, redrawing only what changed), `info`,
  `decompile`, `compile` and `lint`.

Moonglow Viewer contains no game data: it reads the game's files from your
installation.

## Documentation

- [User manual](docs/manual/README.md), also in the app under Help (F1).
- [The plan](docs/PLAN.md): goals, architecture, phases, testing and
  licensing.
- [Packaging](packaging/README.md): the AppImage, Flatpak, Windows
  installer and macOS app.

## Building from source

You need a stable Rust toolchain (1.98 or newer; `rust-toolchain.toml`
selects it with rustup). The Moonglow Toolset crates are fetched from
their git repository (pinned by `Cargo.lock`); while it is private, your
git credentials must reach it (`.cargo/config.toml` has Cargo fetch through
the `git` command line).

```sh
cargo run --release -p moonglow-viewer          # the window
cargo run --release -p mgv -- --help            # the command line
```

## Testing

```sh
cargo test --workspace --release                # corpus and GPU tests skip without a game or GPU
cargo clippy --workspace --all-targets          # must be warning-free
cargo fmt                                       # rustfmt.toml: width 100
```

Tests run at the lowest tier that catches their regressions: unit tests;
corpus tests over the installed game (every compiled model decompiles and
reads back the same; every visual effect applies); differential tests
against nwnmdlcomp; offscreen renders; and the window's flows driven
through `egui_kittest`.

## License

Moonglow Viewer is free software under the
[GNU General Public License, version 3](LICENSE). Game assets, including
Beamdog's shaders, are only ever read from your installation and never
distributed.

Neverwinter Nights is a trademark of its owners. Moonglow Viewer is not
affiliated with Beamdog or Wizards of the Coast.
