# Moonglow Viewer

A standalone, cross-platform (Linux, Windows, macOS) model viewer for
Neverwinter Nights: Enhanced Edition in Rust (egui UI, wgpu renderer),
licensed GPL-3.0: every visual resource type lit as the EE client lights it,
emitters and visual effects, one-click decompile and compile, an ASCII
editor with hot reload, and headless rendering and galleries. The plan,
architecture and phases are in `docs/PLAN.md`.

It builds on the sister project **moonglow-toolset** (`~/Projects/moonglow-toolset`,
github.com/jadzziaa/moonglow-toolset, private): its crates (`mg-core`,
`mg-mdl`, `mg-image`, `mg-resman`, `mg-rules`, `mg-render`, `mg-preview`,
`mg-testkit`, …) are git dependencies pinned by `Cargo.lock`. Their sources
are in `~/.cargo/git/checkouts/moonglow-toolset-*/<commit>/`. Move the pin
with `cargo update -p mg-core` (all move together) and run the whole test
suite.

## Commands

```sh
cargo test --workspace                          # unit tests (corpus and GPU tests skip without a game or GPU)
cargo test --workspace --release                # the same, faster on the corpus
cargo clippy --workspace --all-targets          # must be warning-free
cargo fmt                                       # rustfmt.toml: width 100, "Max" heuristics
cargo run --release -p moonglow-viewer          # the GUI
cargo run --release -p mgv -- --help            # the command-line tool
```

The game's compiler is tested in the toolset's client sandbox, on its
off-screen display (never the desktop), by hand:

```sh
DISPLAY=$(~/Projects/moonglow-toolset/tools/aurora/headless.sh start) \
  cargo test --release -p mgv-mdl --test engine -- --ignored --nocapture
~/Projects/moonglow-toolset/tools/aurora/headless.sh stop   # if you started it
```

Window screenshots for looking at layouts: `cargo test --release -p mgv-ui
--test screens -- --ignored` writes PNGs to `target/test-output/screens/`.

Corpus tests read the installed game (`NWN_ROOT`, or Steam's usual path);
`MOONGLOW_REQUIRE_CORPUS=1` turns their skips into failures. nwnmdlcomp is
found through `NWN_TOOLS_BIN`, `~/.local/opt/neverwinter/bin`,
`~/Projects/neverblender/tools/bin` or `PATH` (`mg_testkit::nwn_tool`).

## Layout

Cargo workspace, layered bottom-up (a crate depends only on crates listed
before it in `docs/PLAN.md` §4): `crates/mgv-mdl` (ASCII writer,
diagnostics, outline, compile and decompile back ends), `mgv-library`
(layers over the game's load order, editor buffers, file watching, model
cache), `mgv-stage` (actors, animation, particles, lights, rigs, camera,
overlays, offscreen rendering), `mgv-gallery` (batch rendering), `mgv-ui`
(the egui app); `apps/moonglow-viewer` (GUI) and `apps/mgv` (CLI);
`packaging/` (release packages) and `docs/manual/` (the user manual).

## Rules

- **Toolset changes stay out of this repository's sessions.** Build on the
  toolset's public APIs here; when something belongs upstream, add it to
  `docs/PLAN.md` §10 (with the viewer's workaround) for the user to land in
  the toolset. Never commit to `~/Projects/moonglow-toolset` from here.
- **Never write to the real NWN user folder** (`~/.local/share/Neverwinter
  Nights`) or the game install. Tests use scratch directories; the engine
  compiler back end always runs with a scratch user directory.
- **Run the game client only through the toolset's sandbox**
  (`~/Projects/moonglow-toolset/tools/nwclient/run-client.sh SCRATCH`, off
  the desktop display), as the toolset's rules say.
- **Game data is never committed**, Beamdog's shaders included: tests read
  the user's install; fixtures contain only data we author.
- **Tests that open a GPU device call `mg_testkit::gpu::hold()` first** (one
  device at a time; many at once coincided with a kernel panic on the
  development machine). Galleries render on one device, in sequence.
- **Parsers and loaders never panic on bad input**; the viewer keeps showing
  the last model that loaded and reports the error.
- **Deterministic output**: decompiled ASCII, galleries and headless renders
  give the same bytes for the same input (particles have a fixed seed;
  headless renders step at fixed intervals).
- Every feature lands with tests at the lowest tier that catches its
  regressions (unit, corpus, differential against nwnmdlcomp, engine, image,
  UI); see `docs/PLAN.md` §6.
- Match the engine's behaviour, not other tools'. Where nwnmdlcomp or
  Neverblender disagree with the game, the game decides; record the
  divergence where the comparison skips it.
