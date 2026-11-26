# Update log

## 2026-11-26
* **Update**: [The plan](PLAN.md) §10.10 and [the command line](manual/05-command-line.md): pictures are framed on the drawn meshes, particles only where no mesh is drawn, as the toolset frames a preview (the owner's ruling).
* **Update**: [The plan](PLAN.md) §10.10 and [lighting and effects](manual/04-lighting-and-effects.md): a held item's emitters are keyed by its `equip` animation as it ends, so a held torch burns (it showed no flame).
* **Update**: [The plan](PLAN.md) §10.10 and [lighting and effects](manual/04-lighting-and-effects.md): the stage plays a preview's overlay (a torch-bearer's raised arm, worn parts with it) and casts its items' Light property, as the toolset's previews do (the owner's ruling).
* **Update**: [The plan](PLAN.md) §10.7 and §10.10: the diagnostics strip's error and warning counts are its header's badges (the kit's `PanelHeader::badge`), after the title, as the owner ruled.
* **Update**: [The plan](PLAN.md) §10.7 to §10.10: the viewer moved onto Moonglow Toolset 5.4.0's kit parts where nothing a user sees changes: the gallery's picture cache is the kit's `PaintedPictures`, the drop preview `stack::paint_drop_preview`, the document tab's close button `shell::doc_tab_close`; the docks' stacking and drop targets, the kept layout and the diagnostics' badges stay the viewer's, with the reasons and each kit gap's state.
* **Update**: [The plan](PLAN.md) §10.10: the toolset's pin is v5.4.0; the preview's torch overlay and item lights are not used yet.
* **Update**: [The plan](PLAN.md) §10.9: moving panels between docks is kept, as the owner ruled.

## 2026-11-25
* **Update**: [The plan](PLAN.md) §10.9: the Right Dock's 352 is ruled (the boards); moving panels between docks awaits the owner's ruling.
* **Update**: [The window](manual/02-the-window.md) and [the plan](PLAN.md) §10.9 (and §10.7's list of what was left): the rest of the redesign (M6 part D): the Timeline's transport row over a full-width scrub bar with a seconds ruler; the Texture's header (channels, Mip, Zoom) and caption line; the Inspector's Creature grid and PLT colors two pairs a row, each color a swatch and a combo opening the palette's 176 colors; panels dragged between the Left, Right and Bottom Docks (or moved by a header's menu); the layout kept between runs, an old or unreadable one opening as the default; the Right Dock opens 352 wide (the viewer boards') so the Creature grid stands two pairs a row.
* **Update**: [The window](manual/02-the-window.md) and [the plan](PLAN.md) §10.8: a model picture with nothing drawn says No picture ("nothing to draw"); the Gallery tab's header puts the slider and count on a row of their own when narrow, never over Find.
* **Update**: [The window](manual/02-the-window.md) and [the plan](PLAN.md) §10.8: a click in a gallery shows the resource as in the list, the arrows step and show; the hints read Click Show and the arrows.
* **Update**: [The window](manual/02-the-window.md), [getting started](manual/01-getting-started.md) and [the plan](PLAN.md) §10.8: Gallery mode (M6 part C): Resources as a List or a Gallery of pictures (the kit's cells and size slider, kept between runs), the creatures' gallery too, a click choosing, a double-click or Enter opening, the arrows moving; Open as Tab puts the gallery in the editor area beside the 3D View; pictures made a few a frame on the window's GPU as `mgv gallery` makes them, textures as themselves, made again after a reload.
* **Update**: [The plan](PLAN.md) §10.7: the Status Bar yields in the toolset's order (the Log's last line to its cap, 96 beside the 3D View and 160 beside Texture and ASCII, before the hints; the hints drop whole; the line is cut to 48, then goes); Effects rows stay on one line, cut short.
* **Update**: [The window](manual/02-the-window.md) and [the plan](PLAN.md) §10.7: the window opens at 1440 by 900 (the owner's ruling, "Viewer width: keep"), its Left Dock folded below 1500 as the toolset's.
* **Update**: [The window](manual/02-the-window.md), [editing models](manual/03-editing-models.md), [lighting and effects](manual/04-lighting-and-effects.md) and [the plan](PLAN.md) §10.7: the 5.x shell (M6 part B): the Top Bar with Open, Save, Decompile, Compile and Compile and View; the Left Dock (Resources, Outliner), Right Dock (Inspector, Lighting) and Bottom Dock (Timeline, Effects, Log), folding in small windows and in two columns from 2560; the 3D View's header with the Overlays popover; the ASCII editor's header, modified dot and diagnostics strip with its counts; the Status Bar with the panel's hints as key caps.
* **Update**: [The window](manual/02-the-window.md), [lighting and effects](manual/04-lighting-and-effects.md) and [the plan](PLAN.md) §10.6: the viewer takes the toolset's 5.x look from `mg-uikit` (theme, fonts, tokens, form helpers; View › Theme, dark or light, kept between runs); the panels Nodes and Animation are now **Outliner** and **Timeline**, and the View menu reads Grid, Axes, Model lights, Game shaders, Shadow (M6 part A).
* **Update**: [The plan](PLAN.md) §10.5: the toolset's pin is v5.3.0, nothing changed in the viewer; GPU suites run with `MG_TEST_GPUS=1`.

## 2026-11-16
* **Update**: [The plan](PLAN.md) §10.4: a worn part's mesh that names its own material or plain texture keeps it (TDoD's head 253 wore its cap drawn in the head's PLT); the viewer's workaround and the change proposed for the toolset's `mg-preview`.

## 2026-11-07
* **Update**: [The command line](manual/05-command-line.md) and [the plan](PLAN.md) §10.3: `mgv clip`, a model and its visual effects as a clip from one camera, with each effect's facts as JSON; [effects](manual/04-lighting-and-effects.md): an instant effect plays once and is gone, and a creature blueprint's effects take its size; the toolset's pin is v3.14.0.

## 2026-11-02
* **Update**: [The window](manual/02-the-window.md) and [the plan](PLAN.md) §10.2: the Shadow switch, the model's shadow on an unseen ground, as the toolset's model viewer has it.
* **Update**: [The plan](PLAN.md) §10.2: the pin is Moonglow Toolset v3.4.0; no shadows drawn; the viewer draws through the framebuffer copies (shiny water by the game's `fswater`, soft particles); what the renderer's changes since 2.8.0 change in the pictures.

## 2026-10-27
* **Update**: [The plan](PLAN.md) §10.2: the pin is Moonglow Toolset v2.8.0; nothing in the viewer needed changing.

## 2026-10-21
* **Update**: [The plan](PLAN.md) §10.2 and §1.1: Moonglow Toolset 2.0 and the game's own shaders, on by default with a switch, compiled in the window while it draws and waited for in pictures, kept in a cache folder, failures said; nwnmdlcomp's decompile back end writes self-illumination in both spellings. [The window](manual/02-the-window.md) (Shaders), [the command line](manual/05-command-line.md) (`--no-game-shaders`) and [troubleshooting](manual/06-troubleshooting.md) say so.

## 2026-10-14
* **Update**: A part a creature wears (a robe, a cloak) plays its wearer's animation at the wearer's size (its skeleton's `setanimationscale`), as the toolset's previews and the game place it: on a dwarf, a gnome or a halfling a robe stood at a human's height, over the head, the feet below its hem (reported by the Domains of Dread session; `mgv-stage`'s `Actor::sample_worn`, held by `a_robe_is_worn_at_its_wearer_s_size`).

## 2026-10-09
* **Update**: [The plan](PLAN.md) §10.1: the toolset took the compiler, the writers, the lint and the outline into `mg-mdl` (its v1.20.5) and binds animations by part number; the viewer is pinned to v1.20.5, its copies are gone, and `mgv-mdl` keeps the external back ends and the tests against nwnmdlcomp, the game's compiler and the client.

## 2026-10-07
* **Update**: [The plan](PLAN.md) §10.1 proposes to the toolset what Phase 9 made: the binary writer, the compiler, the ASCII writer and the lint for `mg-mdl`; what the game client showed against its model notes (animations bound by part number, bounds, skins of more than 18 bones); what that means for `mg-render`'s binding by name; and the window widgets exported.
* **Update**: [The plan](PLAN.md), Phase 9 is done: the native compiler is the default (stage D); [editing models](manual/03-editing-models.md) and [the command line](manual/05-command-line.md) put it first.
* **Update**: [The plan](PLAN.md), Phase 9: stage C is done, skins in the native compiler (up to 64 bones), with what it was checked against; the manual's compiling notes say so.
* **Update**: [The plan](PLAN.md), Phase 9: stage B is done, the native compiler for everything but skin meshes, with what it was checked against; [editing models](manual/03-editing-models.md) and [the command line](manual/05-command-line.md) describe it.
* **Update**: [The plan](PLAN.md), Phase 9: the decompiler writes nodes in the order of their part numbers, so a model without a supermodel keeps them when compiled again; [editing models](manual/03-editing-models.md) says so.
* **Update**: [The plan](PLAN.md), Phase 9: stage A is done (the game client draws models the writer wrote as it draws the originals), and the client showed that the game binds a supermodel's animations by part number, with what follows for compiling a model again.
* **Update**: [The plan](PLAN.md), Phase 9: the binary writer of stage A is in and what measuring it against the game's files showed (mesh boxes hold the origin, part numbers are the file's order, the game ignores function pointers); the open question is now how the game binds a supermodel's animations.
* **Update**: [The plan](PLAN.md) scopes Phase 9, the native model compiler: why (the last use of nwnmdlcomp, which keeps creatures off macOS and at 17 bones a skin), what exists, what is missing, the open questions and four stages with what shows each is done.
* **Update**: [The command line](manual/05-command-line.md) describes `mgv sheet` (contact sheets), `--casters` (shadow casters in place of the visible meshes), `lint --strict` and how pictures are framed; [the window](manual/02-the-window.md) lists the Shadow casters overlay.
* **Update**: Before the repository went public: [the plan](PLAN.md) and the packaging notes no longer describe a private toolset repository (it is public; no token is needed), and nwnmdlcomp is looked for only in `NWN_TOOLS_BIN` and on `PATH` ([editing models](manual/03-editing-models.md)). The README has pictures, in `docs/images/`.
* **Update**: [The plan](PLAN.md) §10 records the move to Moonglow Toolset 1.16.1 (its renderer changes, and the window's labels and headings styled as the toolset's) and the phase table names it for the next release. [The window](manual/02-the-window.md) describes the Inspector's PLT colors and their palettes; [the command line](manual/05-command-line.md) describes `--plt-colors`.

## 2026-10-04
* **Update**: Made `docs/` an Open Knowledge Format (OKF v0.2) bundle: frontmatter on [the plan](PLAN.md) and on every chapter of [the manual](manual/README.md), index files and this log. No content changed. The program's manual reader (`crates/mgv-ui/src/manual.rs`) now skips a chapter's frontmatter.
