# Update log

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
