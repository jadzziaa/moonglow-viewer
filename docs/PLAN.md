---
type: Plan
title: 'Moonglow Viewer: Plan'
description: The plan of Moonglow Viewer with its current status - what it does, the landscape of other tools, principles, architecture, phases, testing tiers, packaging and release, licensing.
tags: [plan, architecture, testing, release]
generated: { by: claude-code/claude-opus-5-5, at: 2026-10-03T07:29:45Z }
---

# Moonglow Viewer: Plan

Moonglow Viewer is a standalone, cross-platform model viewer for Neverwinter
Nights: Enhanced Edition, a sibling of [Moonglow Toolset](https://github.com/jadzziaa/moonglow-toolset).
It opens every visual resource type the game uses and lights it the way the EE
client does, with emitters and visual effects running. It decompiles and
compiles models in one click, edits a model's ASCII source with live reload,
and runs headless for batch renders and galleries. It is written in Rust with
an egui interface and a wgpu renderer, and licensed GPL-3.0.

Decisions taken (2026-10-01):

| Topic | Decision |
| --- | --- |
| Language / UI | Rust (stable, 2024 edition), egui with eframe on wgpu |
| Renderer | The toolset's `mg-render` (EE "enhanced lighting", matched to the client), extended here with overlays, picking and debug views (§4.5) |
| Code reuse | The toolset's crates as a git dependency, pinned in `Cargo.lock`. Viewer work stays in this repository; changes the toolset itself needs are proposed in §10 and landed there when the owner says so (the first set landed 2026-10-01) |
| Model tools | A native decompiler by default; nwnmdlcomp and the game's own model compiler as external back ends; a native compiler later (§4.6) |
| Platforms | Linux, Windows, macOS; the toolset's release targets: AppImage, Flatpak, Windows installer, universal macOS app (§7) |
| Licence | GPL-3.0-only (§8) |
| Oracles | The game install, nwnmdlcomp, the game's model compiler, the game client (sandboxed), neverwinter.nim |

## Status

| Phase | State |
| --- | --- |
| 0 Bootstrap | Done: workspace on the toolset's crates (git, at a toolset release tag; the commit in `Cargo.lock`), lint and format settings as the toolset's, CI on three systems, the plan, `CLAUDE.md` |
| 1 Headless core | Done: `mgv-library` (the game's load order with the opened folder, added folders and archives, editor buffers above; model cache; file watching), `mgv-stage` (actors with play-once, sequences, transitions over `transtime` and attachments, particles, dangly meshes, model lights, the camera, offscreen rendering, deterministic), `mgv render`, `mgv info`. Exit met: a game placeable with emitters renders the same twice; authored models' animation, attachment and reload are tested |
| 2 The application | Done: docked window (3D view, ASCII, texture, resources, nodes, inspector, animation, effects, lighting, log), the orbit camera with framing and views, opening files (dialog, drop, command line) and game resources, creatures by `appearance.2da` row with their look editable, settings kept, crash reports, the manual built in (Help, F1). `egui_kittest` flows cover opening, hot reload, decompiling, saving, creatures and effects |
| 3 Model tools and the ASCII editor | Done but the native compiler (Phase 9): the native decompiler (all 25,597 binary models in the game read back the same, in 6 s; on a sample of 69, nwnmdlcomp's decompile agrees and nwnmdlcomp compiles ours back to the same model), diagnostics checked on the game's 7,235 ASCII models, the outline, the nwnmdlcomp and game-compiler back ends (background jobs, Compile and View), a virtualized editor (0.2 ms idle and 0.25 ms per keystroke at any size; `TextEdit` took 186 ms and 609 ms on `a_ba`), hot reload on edit and on file change, cursor and selection in step with the view. The game's compiler, run in the toolset's client sandbox, compiles the native ASCII of five sample models (placeable, tile, door, effect, animated self-illumination) back to the original model |
| 4 Every visual file type | Done: models, walkmeshes (alone, on their model, and a model's own as an overlay), TGA/DDS/PLT textures with mip levels, channels, TXI and PLT colours, MTR materials on a sphere with their slots and parameters, tilesets (tiles listed, each shown on a click), blueprints (UTC, UTI, UTP, UTD), creatures by appearance row, haks, modules and folders. Not yet: SET tile groups and door placement shown together |
| 5 Lighting, overlays and debug views | Mostly done: studio, `environment.2da` and custom area rigs (day, night, fog, tile main lights from `lightcolor.2da`), overlays drawn in 3D against the scene's depths, faint where it hides them (walkmesh faces by surface material, wireframe, normals; the grid and axes hidden behind models; the node tree and the selection over everything), picking by ray against posed meshes. To do: debug views (unlit, normals, UV checker), skyboxes |
| 6 Emitters and visual effects | Mostly done: every `visualeffects.2da` row with a model applies (503, at three sizes) on hooks found by the target's kind, impact then duration, cessation on removal, from the window and `mgv render --vfx`. To do: `progfx.2da` (beams, node attachments, lights, glows) |
| 7 Batch rendering and galleries | Done: `mgv gallery` over name patterns, 2DAs (placeables, appearance, visualeffects, doors), haks and folders; `index.html` and `manifest.json`, deterministic; re-runs skip unchanged items (all 1,289 placeables render in about 7 s at 256²; an unchanged re-run takes 3 s); `mgv turntable` (animated PNG or frames) |
| 8 Hardening and release | In progress: packaging (the icon; the AppImage, 12.3 MB, built here and its command line run; the Flatpak bundle, 6.8 MB, built here; the Windows installer and macOS app built by the release workflow), the release workflow, the user manual; releases 0.1.0 and 0.1.1 on toolset 0.4.0, 0.1.2 on 0.7.0, 0.1.3 to 0.1.6 on 1.16.1. To do: try the Windows and macOS packages on their systems, performance budgets in CI |
| 9 Native compiler | Done (stages A to D, §5): `mgv compile` and the window compile in process, every kind of model, skins of up to 64 bones, as the game's compiler does and keeping part numbers; every model in the game compiles back to itself, and the client draws the results as the originals. It showed that the game binds supermodel animations by part number |

## 1. What it does

### 1.1 Resources it shows

| Resource | What the viewer shows | Read by |
| --- | --- | --- |
| MDL, binary and ASCII | Every node type (trimesh, skin, danglymesh, animmesh, aabb, emitter, light, reference, dummy, camera), animations with the supermodel chain, events, classification | `mg-mdl` |
| WOK, PWK, DWK | Walkmeshes, faces coloured by `surfacemat.2da`; on their own or over the model of the same name (placeable use points, door open/closed states, tile walkmeshes) | `mg-mdl` (they are ASCII-model shaped) |
| TGA, DDS (BioWare and standard) | The image with its mip levels, channels and alpha, its TXI, and where the game would find it (the layer) | `mg-image` |
| PLT | The ten layers coloured from the `pal_*` palettes, with a colour picker per layer as the toolset's | `mg-image` |
| TXI, MTR | The settings as the game applies them; an MTR's slots and parameters, previewed on its model or on a sphere | `mg-image` |
| Blueprints (UTC, UTI, UTP, UTD) | The object assembled as the game shows it (part-based creatures with equipment, wings and tails; items; placeables with lights; doors) | `mg-preview` |
| 2DA rows | A creature by `appearance.2da` row (phenotype, body parts, colours chosen in the viewer), placeables, doors, item models, `visualeffects.2da` rows | `mg-preview`, `mg-2da` |
| SET | A tileset's tiles listed with their models, doors and walkmeshes | `mg-set` |

Counts in the base game (89.8193.37-17): 32,832 MDL (25,597 binary, 7,235
ASCII), 13,118 WOK, 1,090 PWK, 249 DWK, 28,231 TGA, 6,737 DDS (all BioWare
DXT1/DXT5 but one), 1,565 PLT, 734 TXI, 7 MTR (68 more in `doiwd.hak`, good
PBR test content), 33 SET.

Where resources come from: the game's load order (keys, override,
development), haks, modules and ERFs, and loose folders. Any file opened from
disk brings its folder along as the top layer, so a model next to its textures
shows with them.

Not shown: movies (WBM; EE does not load BIK) and sounds, GUI layouts (GFF
without geometry; GUI *models* are MDLs and are shown), and KTX textures
(ETC2, for mobile; none in the base game; `mg-image` does not read them
yet). Hak shaders (SHD) are not run: the renderer runs Moonglow's own
shaders (toolset plan §5.5), so a hak's custom shaders and an MTR's
`customshaderVS/FS` are named in the inspector only.

### 1.2 What it does with them

- **EE lighting.** The client's enhanced lighting from the toolset's renderer:
  area sun and moon, model and tile lights (`lightcolor.2da`), fog, skyboxes,
  environment and cube maps, MTR maps. Lighting rigs (§4.3) choose between a
  neutral studio light and the game's own area lighting.
- **Emitters and visual effects.** Every emitter type the renderer simulates
  runs in the viewer; visual effects from `visualeffects.2da` attach to a
  creature's hooks and play their impact, duration and cessation (§4.4).
- **One-click model tools.** Decompile a binary model to ASCII, compile ASCII
  to binary, and compile-and-reload, through the back end the user picks
  (§4.6).
- **ASCII editor with hot reload.** The decompiled (or original) ASCII opens
  beside the 3D view; every edit, and every change made to the file by another
  program, reloads the model in place, keeping the camera, animation and
  selection (§4.7).
- **Headless.** The command-line tool renders stills and turntables, builds
  galleries (thumbnails with an HTML index and a JSON manifest), decompiles
  and compiles, and reports model information as JSON (§4.8).

## 2. Landscape

Model viewers for NWN today (surveyed in the toolset's
`docs/research/prior_art.md`): NWNExplorer (Windows, legacy fixed-function
rendering), Borealis MDL (C++/Qt, GPL-3.0, binary and ASCII with a
decompiler), dunahan's three.js web viewer (MIT; PBR through three.js, not EE
parity), rollnw's client (C++, viewer-first) and Neverblender (Blender import
and export). None reproduces EE's lighting, none edits ASCII with live reload,
and none renders headless galleries. The toolset's renderer already matches
the client's lighting within 1/255 per region on its reference scenes; the
viewer builds on it.

## 3. Principles

The toolset's principles, applied to a viewer:

1. **Lean.** Reuse the toolset's crates instead of copying them; few
   dependencies; one GPU device per process; small crates with one job each.
2. **Modern.** Current stable Rust, wgpu and egui; EE formats (MTR, PBR maps,
   EE skins, normals and tangents in ASCII) are first-class, not afterthoughts.
3. **Performant.** Memory-mapped archives, models parsed once and cached by
   name, textures decoded on first use, work done off the UI thread where it
   is slow (galleries, external compilers). Budgets are tested (§6).
4. **Compatible.** The engine is the reference. ASCII the viewer writes loads
   in the game, in nwnmdlcomp and in Neverblender; what it renders matches the
   client; resources resolve in the game's load order.
5. **Headless first.** Everything the window does is a call into a UI-free
   crate: the window renders and issues calls; the command line and tests use
   the same calls.
6. **Game data is never shipped.** The viewer reads the user's install at run
   time; test fixtures contain only data we author. Beamdog's shaders are
   game data too: the renderer reimplements their lighting, and their text is
   never copied into the repository or the packages.
7. **Never write where the user did not ask.** Nothing is written to the game
   install or the NWN user folder unless the user picks it as a destination;
   decompiled and compiled files go next to their source or to a folder the
   user chose. Hot reload never touches the file on disk; only Save does.
8. **Deterministic output.** The same input gives the same bytes: galleries,
   decompiled ASCII and rendered images (particles use a fixed seed and fixed
   time steps).
9. **Parsers never panic.** Bad input gives a message, never a crash; the
   viewer keeps showing the last model that loaded.

## 4. Architecture

A Cargo workspace on top of the toolset's crates. A crate depends only on
crates above it in this table.

| Crate | Responsibility |
| --- | --- |
| toolset: `mg-core`, `mg-2da`, `mg-gff`, `mg-erf`, `mg-key`, `mg-set` | Formats |
| toolset: `mg-mdl`, `mg-image` | Models (binary and ASCII readers), textures, TXI, MTR |
| toolset: `mg-resman`, `mg-rules` | The game's load order; 2DA and talk-table data |
| toolset: `mg-render`, `mg-preview` | The renderer (lighting, animation, skinning, dangly meshes, particles); blueprint previews |
| `mgv-mdl` | Model source tools: the ASCII writer (decompiler), diagnostics, the node outline of an ASCII file, keyword tables, and the compile and decompile back ends |
| `mgv-library` | Where things come from: the install, extra folders, haks, modules and the opened file's folder as layers; editor buffers as an in-memory layer; file-type detection; watching files for changes; caches of parsed models |
| `mgv-stage` | What is shown, without a window: actors (model instances with their animation, particles, dangly meshes and lights) hanging from each other's nodes; blueprints, 2DA rows and visual effects turned into actors; lighting rigs; walkmeshes; the overlay renderer and picking; the camera; stepping time; offscreen rendering of stills and turntables |
| `mgv-gallery` | Batch rendering: a selection of resources to images, an HTML index and a JSON manifest |
| `mgv-ui` | The egui application |
| `apps/moonglow-viewer` | The GUI |
| `apps/mgv` | The command-line tool |

Tests use the toolset's `mg-testkit` (corpus locator, oracle tools, the GPU
lock).

### 4.1 The library

`mgv-library` builds a `mg_resman::ResMan` for the session: the game's layers
(`ResMan::for_game`), then haks and modules the user adds, then extra folders,
then the opened file's folder (priority 99, class Directory, so its textures
beat override's), then an in-memory layer holding editor buffers (above
everything). Opening `C:\work\foo.mdl` therefore finds `foo.tga` and `foo.mtr`
beside it, and editing `foo.mdl` in the viewer shows the edit without saving.

Parsed models are cached by name. A change (an edit, a file written by another
program, a layer added) invalidates what depends on it: the model, its
supermodels, its textures (`Renderer::clear_textures`), and the 2DA cache
(`GameData::invalidate`). Files are watched with the platform's notifications
(`notify`), debounced.

File types are detected by extension and checked by content (binary models
start with four zero bytes; DDS, TGA and PLT headers), so a misnamed file
still opens as what it is.

### 4.2 The stage

A stage is a tree of actors. An actor is a model instance with:

- its model (`GpuModel`) and the animations of its supermodel chain;
- an animation player: the current animation, time, speed, looping or once
  (the renderer's `pose` always loops, so play-once clamps the time), and an
  optional transition (`transtime`) blended from the previous pose;
- its particles (`mg_render::particles::Particles`), dangly state, mesh state
  (animated alpha, self-illumination, animmesh vertices) and lights;
- where it hangs: another actor's node (by name) with a scale, or the world.

A plain model is one actor. A blueprint or 2DA row becomes a base actor and
its parts (from `mg_preview::Preview`; unlike the toolset's `Composed`, every
part gets its own particles and dangly state). A visual effect becomes actors
hanging from the target's hook nodes. Stepping the stage by `dt` advances
every actor and produces a `mg_render::Scene` plus the overlay geometry.

Time is explicit (`step(dt)`), so headless renders choose their frame times
and the window feeds its frame time.

### 4.3 Lighting rigs

A rig turns into the scene's area light, point lights, fog, background, sky
and environment map:

- **Studio** (default): a neutral key light from the front-left, soft ambient,
  a neutral background; the model's own lights on.
- **Area**: sun or moon ambient and diffuse, fog and sky from an
  `environment.2da` row (or values typed in, as an area's properties hold
  them), for day, night, dawn and dusk; tile main and source lights coloured
  from `lightcolor.2da`.
- **Custom**: any of the above plus extra point lights placed in the view.

The model's light nodes (with their animated colour and radius) light the
scene unless switched off; up to `MAX_LIGHTS` (32) per draw by priority and
distance, as the renderer chooses them.

### 4.4 Visual effects

`visualeffects.2da` rows attach their models to the target's hooks
(nwn.wiki, Model Special Nodes):

| Column | Creature | Placeable | Door |
| --- | --- | --- | --- |
| `Imp_HeadCon_Node` | `head` | `<name without its first 4 characters>_head_hit` | `hhit` |
| `Imp_Impact_Node` | `impact` | `_impact` | `impc` |
| `Imp_Root_{S,M,L,H}_Node` | `root`, by `appearance.2da` `SIZECATEGORY` (1–2 S, 3 M, 4 L, 5 H) | `_ground` | `grnd` |

`Type_FD` is F (instant), D (duration), P (projectile) or B (beam); the
`Ces_*` columns are unused in the shipped table. A model plays `impact`,
then loops `duration` (D rows), and plays `cessation` when the effect is
removed; instant ground effects last about 2 s, and emitters in rows other
than D and B stop after about 1 s. `conjure01`, `cast01` and `travel01`
models play those (toolset `notes_models.md` C1.7). `OrientWithObject`
rows follow the hook's rotation, others only its position.

The target is the opened model or a stand-in creature (any `appearance.2da`
row). Programmed effects (`progfx.2da`, types 1–13) come in later, the
useful ones first: beams (type 7: the beam model's point-to-point and
lightning emitters aimed at a second actor's `impact` each frame), node
attachments (12), lights (4: `fx_light_clr` with an animation such as
`White_10m`), envmap, glow, colour pulse and alpha cycle (2, 3, 5, 6), and
freezing animation (13). Sounds and screen shake are not played.

### 4.5 Overlays, picking and debug views

`mg-render` draws the lit scene into targets the viewer owns, then the viewer
draws overlays into the same targets with its own pipeline (lines, points and
translucent faces): the ground grid and axes, bones and hook nodes, normals,
wireframe, bounding boxes, walkmesh faces by surface material, light radii,
emitter gizmos and the selection outline.

The renderer keeps its depth buffer (§10), so the overlays (`mgv_stage::overlay`)
are tested against the scene: what it hides shows at 30%, so a walkmesh
under a floor or bones inside a body are still seen; the node tree and the
selection's box show over everything; names and node markers are painted on
top. The viewer poses meshes on the CPU (rest pose, animation, skinning,
animmesh and dangly vertices) for the overlays' geometry and for picking:
the mouse ray against the posed triangles selects the node, which selects
it in the outliner and the ASCII editor.

Debug views: lit (default), unlit textures, the material's diffuse colour
(`DebugView::MaterialDiffuse`), normals as colours, and UV checker.

### 4.6 Model tools

Decompile and compile go through back ends:

| Back end | Decompile | Compile | Where | Notes |
| --- | --- | --- | --- | --- |
| Native | ✓ (default) | Phase 9 | All platforms, in process | Writes ASCII from `mg-mdl`'s model of a binary file: lossless for what binaries hold |
| nwnmdlcomp | ✓ | ✓ | Linux (32-bit build), Windows | Torlack's compiler (BSD), found in `NWN_TOOLS_BIN` or on `PATH`. Predates EE: drops `normals` and `tangents`, drops `materialname` and `renderhint` silently (the viewer refuses, as Neverblender does), stores Bézier keys in a layout the game reads differently (refused too), takes only 0 and 1 for `spawntype` (EE-compiled models hold −1: given to it as 0, drawn alike), and allows 17 bones per skin (EE: 64). Decompiles EE-compiled models correctly. Always exits 0; errors are `Error:` lines |
| Engine | — | ✓ | Linux, Windows, macOS (the game ships `nwmain` for all three) | The game's own compiler: `nwmain -userdirectory SCRATCH compilemodel RESREF` with the model staged in the scratch user directory's `development/`, the result collected from `modelcompiler/`. Keeps EE features. Needs an OpenGL context: on Linux it runs inside `gamescope --backend headless`; elsewhere a window opens briefly. Cannot compile skin meshes from the command line. Success is the log's "Successfully compiled model" plus a fresh file. A scratch user directory always, never the real one |

Compile picks a back end per model the way Neverblender's `nwn_compile.py`
does (engine unless the model has skin meshes, else nwnmdlcomp), unless the
user chose one. Outputs go next to the source (`compiled/`) or to a chosen
folder; nothing is overwritten without asking. Compiler messages go to the
log with their lines.

Decompiled ASCII follows nwnmdlcomp's layout (Neverblender reads it), with
EE fields (`materialname`, `renderhint`, `normals`, `tangents`, Bézier keys)
kept where the source has them: the game's compiler stores `materialname`
in the `texture3` slot and generates tangents for a `renderhint`, which
`mg-mdl` reads back. Self-illumination is written under both spellings,
`setfillumcolor` (the only one nwnmdlcomp compiles) and `selfillumcolor`
(the only one the game's compiler keeps; found by compiling in the game),
at rest and keyed; each compiler skips the other's.

### 4.7 The ASCII editor and hot reload

The editor shows an ASCII model with highlighting (keywords by node type,
numbers, comments), the node outline, diagnostics and search. Edits reload the
model after a short pause (debounced, ~150 ms): the buffer goes into the
library's in-memory layer, the model is parsed again (`a_ba`, the game's
largest ASCII model at 126,000 lines, parses in 41 ms), and the stage swaps
the actor's model, keeping camera, animation, time and selection.

`mg-mdl`'s reader is as lenient as the game (it skips what it does not know
and never fails); its source map places every node of the model it reads,
which keeps the cursor and the selection in step even where names repeat.
Diagnostics come from `mgv-mdl`'s own pass over the text:
unknown keywords, list counts that do not match, indices out of range,
parents that do not exist, keywords nwnmdlcomp would drop. They are marked
in the text and listed in the log.

Selection follows both ways: the cursor in a node's block selects the node in
the view and outliner; picking a node in the view moves the editor there.
Saving writes the file (keeping its line ends); a file changed on disk while
it has no unsaved edits reloads by itself, otherwise the viewer asks.

Large files: egui's `TextEdit` lays out the whole text on each change (the
toolset measured ~10 ms per keystroke at 14,000 lines). Decompiled creature
models run 10,000–25,000 lines and `a_ba` 126,000, so the editor is a
virtualized widget (only visible lines laid out, a line-indexed buffer), with
`TextEdit` kept only if the Phase 3 spike shows it fast enough.

### 4.8 Headless operation and galleries

`mgv` (the command-line tool) uses the same crates as the window:

- `mgv render MODEL|FILE [--anim A --time T] [--camera …] [--rig …] -o out.png`
- `mgv turntable MODEL --frames N -o DIR` (PNG frames; animated PNG)
- `mgv gallery SELECTION -o DIR`: SELECTION is a resource-name pattern, a
  2DA (`placeables.2da`, `appearance.2da`, `visualeffects.2da`), a hak or
  module, or a folder; writes thumbnails, `index.html` and `manifest.json`
  (name, layer, classification, nodes, meshes, animations, textures, bounds).
  Parsing runs in parallel; rendering uses one device. Re-running skips
  images whose inputs have not changed.
- `mgv decompile|compile FILE… [--backend …] [-o DIR]`
- `mgv info MODEL` (JSON), `mgv texture IN -o out.png`, `mgv which|ls` for the
  resource stack.

Headless rendering needs a GPU adapter but no display (Vulkan, Metal, DX12,
or GL as a fallback).

### 4.9 The window

A 3D view in the middle, the resource browser and outliner on the left, the
inspector on the right, the animation timeline and log at the bottom, and the
ASCII editor as a tab beside the view (docked with `egui_dock`, so it can sit
side by side). The camera orbits, pans and zooms as in the toolset's model
viewer, with framing, front/side/top views and a fly mode. Views are taken
from the model's front: creatures face +Y, but the game's placeables face
−Y (armoires' doors, chairs' and thrones' seats, a standing mirror's glass;
checked on 16 common placeables), though Neverblender's notes say all
models face +Y, and use points do not tell (a standing mirror's is behind
its glass). Tiles and effects keep +Y. `--yaw` stays absolute. Open by
File › Open, drag and drop, the command line, or the browser.

## 5. Phases

Each phase ends with a demonstrable result and green tests.

### Phase 0: Bootstrap
Workspace on the toolset's crates (git, pinned), lint and format settings as
the toolset's, CI on three systems, the plan and `CLAUDE.md`.

### Phase 1: Headless core
`mgv-library` (install, layers, the opened file's folder, model cache),
`mgv-stage` (actors, animation player, particles, dangly meshes, model lights,
the studio rig, the camera, framing), offscreen rendering; `mgv render`,
`mgv info`.
**Exit:** every model in the game loads into a stage and steps a second of
its idle animation without error; renders of reference models match golden
images.

### Phase 2: The application
eframe shell with docked panels, the 3D view (orbit, pan, zoom, framing,
views), opening files and game resources, the resource browser, outliner,
inspector, animation timeline, screenshots, settings kept between runs.
**Exit:** `egui_kittest` flows: open a model from disk and from the game,
play an animation, select a node.

### Phase 3: Model tools and the ASCII editor
The native decompiler, the nwnmdlcomp and engine back ends, one-click
Decompile, Compile and Compile & reload; the editor with highlighting,
diagnostics, outline and selection sync, hot reload on edit and on file
change.
**Exit:** every binary model in the game decompiles natively, and the ASCII
reads back to the same model; nwnmdlcomp compiles the native ASCII of a
sample; an edit in the editor shows in the view within one reload.

### Phase 4: Every visual file type
Textures (mip levels, channels, alpha, TXI), PLT colouring, MTR, walkmeshes
alone and over their models, blueprints, 2DA rows (creatures by appearance
with phenotype, parts and colours), tilesets, GUI models; haks, modules and
folders as sources.
**Exit:** every texture, walkmesh and tileset in the game opens; previews of
every blueprint type.

### Phase 5: Lighting, overlays and debug views
Lighting rigs (studio, area from `environment.2da`, custom), tile lights,
fog, skyboxes; the overlay renderer, picking, debug views.
**Exit:** an area-rig render of a tile matches the toolset's area view of the
same tile and light; picking selects every node type.

### Phase 6: Emitters and visual effects
The emitter inspector (live parameters), restart and burst; the
`visualeffects.2da` browser applying effects to a target with
impact/duration/cessation; progfx beams and node attachments.
**Exit:** every `visualeffects.2da` row with a model applies to a stand-in
creature without error; a sample matches client screenshots of the same
effect.

### Phase 7: Batch rendering and galleries
`mgv gallery`, turntables, incremental re-runs, the HTML index.
**Exit:** a gallery of every placeable in `placeables.2da` within the
performance budget; identical bytes on a second run.

### Phase 8: Hardening and release
Packaging (§7), the user manual (in the app under Help), crash reports,
performance budgets in CI, releases built by CI.

### Phase 9: Native compiler
ASCII to binary in process, on every platform, keeping EE features where the
binary format holds them; checked against nwnmdlcomp's output, the engine
compiler's, and in the game. Scoped on 2026-10-07; stages A (the writer,
`mgv-mdl/src/binary.rs`), B and C (`compile.rs`) and D (the default) are
done, each checked in the game.

**Stage A so far.** All 25,597 compiled models in the game, read, written
and read again, are the same `Model`, and nwnmdlcomp decompiles a sample of
the written files (skins, dangly and animated meshes, emitters, walkmeshes
among them) as it does the originals. What the writer derives was set
against the bytes BioWare's compiler and nwnmdlcomp wrote: face normals and
plane distances agree in 99.9% of 8 million faces, the faces beside each in
99.3%, mesh boxes in 99.6%, radii and averages in 94%. Three things that
measuring showed:

- A mesh's box holds its origin as well as its vertices, and its average
  counts a place once however many corners meet there.
- Part numbers are the nodes' order in the *file* the compiler read, which
  a compiled model's tree does not keep: in 4,343 of 22,787 models without
  a supermodel they are a permutation of the tree's order. A node the
  supermodel has at the same place by name takes its number; one it lacks
  there has −1; what is under such a node keeps its own. So a supermodel's
  numbers are read from its compiled file (`PartNumbers::read`), not
  derived.
- The game ignores the function pointers: its own compiler leaves run-time
  values there, in files the game ships.

**In the client** (`mgv-mdl/tests/client.rs`, by hand on the toolset's
off-screen display: a scratch module, the toolset's client sandbox, a
screenshot of its window). An armoire, a wererat, a sitting man and a red
dragon, with their supermodels, eight models in all: written again and put
in the scratch user folder's `development`, the game draws the scene as it
does with its own files (the pictures differ by 0.05 a channel, as two runs
of the originals do). Stage A is done.

The same test answered the open question. **The game finds a supermodel's
animations for a model's nodes by part number, not by name**: with `a_ba`
and its supermodels written again under numbers a compiler would give them
afresh, the man no longer sits as he did. So:

- A model compiled again must keep the part numbers its compiled version
  has, wherever other models are compiled against it. The native compiler
  will take them from the compiled model of the same name it finds (a
  node of the same name at the same place keeps its number; new nodes get
  numbers past the old count), and only number from scratch a model that
  is new.
- Through ASCII, a compiler numbers nodes in the order of the text. So the
  decompiler writes a compiled model's nodes in the order of their part
  numbers (`write::file_order`; a parent still before its children), as
  nwnmdlcomp's does for a model without a supermodel: of 24,298 such
  models in the game, 5,079 are numbered in another order than their
  trees', and all keep their numbers through the text; nwnmdlcomp compiles
  it back to the numbers they had. Kept in the tree's order instead: a
  model with two nodes of one name (text tells them apart only by a child
  following its parent), and one whose skin would be written before a bone
  of its own (nwnmdlcomp knows only the nodes it has read by then).
- With a supermodel of its own (`a_ba` has one) no text keeps the numbers:
  BioWare's have gaps there. That is for the native compiler's rule above.

**Stage B** (`mgv-mdl/src/compile.rs`; `mgv compile --with native`, and
Model › Compile with › Moonglow in the window; not Automatic's choice yet).
`mg-mdl`'s ASCII reader already makes of a text what a compiled model holds
(a vertex per corner, normals from smoothing groups), so the compiler adds
little: tangents for a render hint given without, a walkmesh's tree where
the text has none, a shininess of 1 where the text does not say, animated
sets spread over the render vertices, controllers a node's type does not
have left out with a note, and the part numbers: by the order of the text,
from the supermodel, and from the compiled model of the same name where
there is one.

- All 24,874 compiled models in the game without skin meshes or two nodes
  of one name, decompiled and compiled again, are the models they were,
  with the part numbers and node counts they had; the 24,298 whose
  numbers are their own get them back from the text alone.
- Against the game's own compiler on the same text: the five models of
  the engine test read back the same from both, and for a cube that
  leaves normals and tangents to the compiler, both give the same 24
  vertices, with normals and tangents equal to four decimals (so the
  game's normals are area-weighted, as the reader's are, and its tangents
  the usual ones).
- In the client: the scene's six models without skins, compiled from
  their decompiled text, `a_ba` and its supermodels among them, are drawn
  as the originals, the man sitting on them as before.
- A model compiles in about a tenth of a second, against a second and a
  half through the game.

**Stage C** (skins). A text's skin names its bones per vertex, which the
ASCII reader already resolves; what a compiled skin holds besides is where
each node of the model sees the skin from at rest (a rotation and a
translation a node, `compile::inverse_binds`), and the bone tables the
writer makes. More than 18 bones are written in EE's larger structure.

- All 355 compiled models with skins in the game compile back from their
  decompiled text to the models they were (weights by bone, numbers), so
  all 25,227 models without two nodes of one name do.
- The inverse binds made from the model at rest are those of the files
  for 4,514 of 4,594 bones. The rest are one family, ten `c_wingdrg2_*`
  wing models, bound in another pose than they rest in, which no text
  holds.
- In the client: the red dragon and its supermodel compiled natively are
  drawn as the originals; and Neverblender's test mannequin, 52 bones a
  skin, lying posed where the wererat lay, is drawn the same compiled
  natively as from its text (which the game reads itself). So EE's
  64-bone structure, as the writer lays it out, is what the game takes.
- Dangly meshes, animated meshes and walkmeshes came with stage B: the
  reader had them whole.

**Stage D** (the default). `mgv compile` and the window's Compile use the
native compiler unless another is chosen (`--with engine`, `--with
nwnmdlcomp`; Model › Compile with); `auto` still reads, and means it.

- The game's own 7,235 ASCII models (a fifth of its models are text) all
  compile, in 3 seconds on this machine's cores, each to the model its text
  reads as.
- Neverblender's compile tool takes it as its default, and its end-to-end
  run passes on it: placeables, a skinned column, an FBX chest, two
  mannequins and a wolf, with no game started to compile and no
  nwnmdlcomp.

Left for later, on purpose: the game's compiler and nwnmdlcomp stay as
back ends (they are the oracles the tests compare with); the writer and
the compiler stay in `mgv-mdl` until proposed to the toolset (§10).

**Why now.** It is the last thing that keeps nwnmdlcomp in use: the game's
own compiler cannot compile skin meshes, so every creature goes through a
32-bit program from 2018 that is built from source on Linux, does not run on
current macOS, allows 17 bones a skin (EE: 64) and drops normals, tangents,
`materialname` and `renderhint`. Neverblender, which is being made
cross-platform, depends on it for creatures.

**What there is to build on.**

- `mg-mdl` reads the binary format completely (`binary.rs`, about 730
  lines; the layout from nwnmdlcomp's headers, checked on all 25,597
  compiled models in the game) and reads ASCII into the same `Model`.
- The native decompiler writes that `Model` as ASCII, and every compiled
  model in the game reads back the same through it.
- Three oracles are already wired into the tests: nwnmdlcomp, the game's
  compiler, and the game client in its sandbox.

**What is missing** is the other direction, in two parts.

1. *A binary writer*: a `Model` laid out as the 32-bit memory dump the game
   loads. Headers, the name table, nodes with their type headers
   (mesh, skin, dangly, animmesh, aabb, light, emitter, reference),
   controller keys and data (Bézier keys included), animations with their
   node trees and events, and the raw data section with its offsets.
2. *What a compiler derives* that ASCII does not hold and a binary must:
   - vertices split or merged from `verts`, `tverts` and `faces` into
     per-vertex arrays, and the index list that is drawn;
   - vertex normals from smoothing groups, and tangents where a
     `renderhint` asks for them;
   - per-face normals, plane distances and adjacent faces (shadows), each
     mesh's bounds, average and radius, and the model's;
   - a walkmesh's AABB tree;
   - a skin's bone table, its four weights a vertex, and each bone's
     inverse bind rotation and translation;
   - dangly constraints and animmesh sample data;
   - node numbers, which must agree with the supermodel's for its
     animations to apply, so the supermodel is read first.

**Open questions**, each to be answered by measurement before the stage
that needs it:

- None left of those the scope listed: the game's normals and tangents are
  the reader's (stage B), 52 bones a skin work in the layout written
  (stage C; 64 is the documented limit, untried).

**Stages**, each ending with a result that can be shown:

| Stage | What | Shown by |
| --- | --- | --- |
| A | The binary writer, fed with models read from binaries | Every compiled model in the game, read, written and read again, is the same `Model`; a sample of the written files loads in the client and draws as the original |
| B | Derivations for meshes without bones; lights, emitters, references, animations | Decompiled game models compiled natively read back as the originals; the same ASCII through the game's compiler reads back the same (the differential the tests already make with nwnmdlcomp) |
| C | Skins, dangly meshes, animmeshes, walkmeshes | Creatures compiled natively and by nwnmdlcomp read back alike; a creature with more than 17 bones a skin animates in the client; Neverblender's end-to-end run passes without nwnmdlcomp |
| D | `mgv compile --with native`, then the default; the window's Compile; the manual | The corpus compiles in a time worth stating; nwnmdlcomp and the game's compiler stay as choices |

A is about the size of the reader. B and C are each larger than A, and C
carries the most uncertainty (skins are where nwnmdlcomp and the game are
least documented).

**Where it lives.** In `mgv-mdl`, beside the ASCII writer, on `mg-mdl`'s
`Model`: the viewer's side of the line (§10). It belongs in `mg-mdl` in the
end, where the toolset could use it too; proposed there once it has passed
stage B.

**What it is not.** Not a re-implementation of nwnmdlcomp's output byte for
byte: the measure is the game (the model loads, draws and animates the
same), and reading back to the same `Model`.

## 6. Testing

| Tier | What | Runs |
| --- | --- | --- |
| L0 unit | Per crate; the ASCII writer and diagnostics on authored snippets; truncation tests | Always |
| L1 corpus | Every model in the install loads into a stage; every binary decompiles and reads back the same; every texture decodes | When a game install is found |
| L2 differential | nwnmdlcomp compiles native ASCII, and its decompile of that reads back the same; native ASCII equals nwnmdlcomp's ASCII after parsing | When nwnmdlcomp is found |
| L3 engine | The engine compiler compiles native ASCII; compiled models load in a sandboxed client | Linux, when the game is installed |
| L5 image | Offscreen renders against golden images; client screenshots for lighting and effects | GPU runners / locally |
| L6 UI | `egui_kittest` flows | Always |

Budgets (from the first measurements, only tightened): open the largest
binary model, reload `a_ba` from an edit, render a 1024² still, gallery
throughput.

Tests that open a GPU device call `mg_testkit::gpu::hold()` first (one device
at a time; many at once coincided with a kernel panic on the development
machine). Galleries use one device and render in sequence.

## 7. Packaging and release

The toolset's packaging, adapted: an AppImage and a Flatpak for Linux, an Inno
Setup installer for Windows (per-user by default), a universal macOS app in a
disk image; licence notices for third-party crates; a release workflow that
builds the three CI packages on a tag and drafts the release. App ID
`io.github.moonglow_toolset.MoonglowViewer` (macOS:
`io.github.moonglow-toolset.MoonglowViewer`); binaries `moonglow-viewer` and
`mgv`; `.mdl` registered as a model type. The Flatpak's source generator
learns git sources (the toolset's crates).

## 8. Licensing

GPL-3.0-only, as the toolset (whose crates it builds on). nwnmdlcomp is
BSD-licensed (Torlack / OpenKnights `_NwnLib`), so it could be bundled; the
viewer finds it instead, since it only builds as a 32-bit program and cannot
run on current macOS. Beamdog's shaders and all game assets are read from the
user's install and never redistributed.

## 9. Risks

| Risk | Mitigation |
| --- | --- |
| Editor speed on 20,000–126,000-line files | Virtualized editor widget (§4.7); a Phase 3 spike measures `TextEdit` first |
| Native decompiler misses something nwnmdlcomp writes | Corpus round trips through both readers, differential tests against nwnmdlcomp |
| The engine compiler needs a display, and fails on skin meshes | gamescope headless on Linux; nwnmdlcomp for skins; clear messages |
| Particle details differ from the game (mid values, bounce, wind, splat are not simulated) | Client comparisons per emitter type; fixes proposed upstream (§10) |
| The toolset changes its APIs | Pinned commit; the pin moves deliberately, with the test suite |

## 10. Toolset changes

Changes that belong in the toolset's crates are proposed here and landed
there when its owner says so. The first set landed on 2026-10-01 (toolset
`32fa513`), each settled where it could be against the game: probe models
compiled by `nwmain compilemodel` and particles measured in the sandboxed
client (the toolset's `notes_models.md` B.8, B.20, B.20a).

| Crate | Change | Viewer use |
| --- | --- | --- |
| `mg-render` | The depth buffer kept after a frame (`DEPTH_FORMAT`) | Overlays tested against the scene (§4.5) |
| `mg-render` | Normal maps on a model's own tangents (the stock shaders' frame), else screen-space derivatives | Drawn as the game draws them |
| `mg-render` | Particles: `bounce` (measured: 0.8 of the speed kept, × `bounce_co` off the ground) and `m_isTinted` (the light at the emitter; the area's part matches the client exactly); gravity confirmed (mass × 9.8) | Effects and creatures |
| `mg-render` | Animations played once hold their last frame; `anim::locals`, `compose` and `blend` for transitions | Players hold at the length; transitions over the new animation's `transtime` |
| `mg-mdl` | Source maps (`ascii::read_mapped`): each node's and animation's lines, notes on what the reader skipped | Cursor and selection sync by node index |
| `mg-mdl` | What the game's compiler writes, read back: `materialname` (texture3 slot), `renderhint` (+0xE4), tangents, Bézier keys (value and two handles), the emitters' three-stop IDs (448–472, not nwnmdlcomp's) | Lossless decompile |
| `mg-mdl` | ASCII integers as C reads them (`spawntype -1` is 0xFFFFFFFF, as compiled) | The decompiler writes −1; the nwnmdlcomp back end gives that compiler 0 |
| `mg-mdl` | Walkmesh files (`walkmesh::Walkmesh`): nodes under the object's root, surfaces per door state, use and door points | Walkmeshes alone and on their models |
| `mg-resman` | Layers changed in place (`ResMan::rescan`, `replace`) | Rescans and editor buffers keep each layer's place |
| `mg-preview` | Creatures by appearance alone (`CreatureLook`, `creature_look`) | Creature browsing (the right foot, which the viewer's own blueprint fields missed) |

Landed since, in v0.7.0: a coloured texture name takes
the PLT before a TGA or DDS of the name (`mg-render`, toolset `bdd5818`;
the game colours a dwarf's head, a PLT and a plain gray TGA, with the
creature's skin), and a body part whose mesh names a texture that does not
exist takes the one of the part model's own name (`mg-preview`, toolset
`8b7e31f`; `pfh0_belt063` names `beltmerged`). The viewer's wrappers for
both are gone; `mgv_stage::textures::part_fallbacks` stays for a part
model opened on its own, which is no preview.

The viewer's pin is v1.16.1 (2026-10-07), and with it the toolset's
renderer as of that release, with nothing to adapt but the scene's clock
(`Scene::time`, the stage's elapsed time, for the water's ripples):
vertex colours and a second UV set for the community tileset shaders
`vertexalpha` and `mzlm`, drawn by the toolset's own shader when a
material names them; see-through meshes drawn in two parts; water (TXI
`proceduretype arturo` and `bumpmaptexture shinywater`, approximated);
linked particles; a static placeable with no visual transform and a TGA's
right-to-left bit ignored, both measured in the client. The window
follows the toolset's forms: field labels in the strong colour, section
headings in Ubuntu Bold over a rule, palette swatches for PLT colours
(`mgv-ui/src/widgets.rs` and `palette.rs`, copies of the toolset's, which
its interface crate does not export).

Not taken, because the game does not do them: UV sets 2 and 3 (no
shader reads them; the toolset has since taken vertex colours and UV set
1, for the community shaders above); the emitters' three-stop
values (`colorMid`, `alphaMid`, `sizeMid`, `percentStart/Mid/End`: the
client draws start to end whatever they hold); anything for `twosidedtex`
(one-sided aligned particles show from both sides). Not measured, so not
simulated: wind on particles, `splat`, `deadspace`, and the point lights'
exact share of a tinted particle's light (within 20/255 in the one scene
measured).
