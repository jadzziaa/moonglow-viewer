# Update log

## 2026-10-07
* **Update**: [The plan](PLAN.md), Phase 9: stage A is done (the game client draws models the writer wrote as it draws the originals), and the client showed that the game binds a supermodel's animations by part number, with what follows for compiling a model again.
* **Update**: [The plan](PLAN.md), Phase 9: the binary writer of stage A is in and what measuring it against the game's files showed (mesh boxes hold the origin, part numbers are the file's order, the game ignores function pointers); the open question is now how the game binds a supermodel's animations.
* **Update**: [The plan](PLAN.md) scopes Phase 9, the native model compiler: why (the last use of nwnmdlcomp, which keeps creatures off macOS and at 17 bones a skin), what exists, what is missing, the open questions and four stages with what shows each is done.
* **Update**: [The command line](manual/05-command-line.md) describes `mgv sheet` (contact sheets), `--casters` (shadow casters in place of the visible meshes), `lint --strict` and how pictures are framed; [the window](manual/02-the-window.md) lists the Shadow casters overlay.
* **Update**: Before the repository went public: [the plan](PLAN.md) and the packaging notes no longer describe a private toolset repository (it is public; no token is needed), and nwnmdlcomp is looked for only in `NWN_TOOLS_BIN` and on `PATH` ([editing models](manual/03-editing-models.md)). The README has pictures, in `docs/images/`.
* **Update**: [The plan](PLAN.md) §10 records the move to Moonglow Toolset 1.16.1 (its renderer changes, and the window's labels and headings styled as the toolset's) and the phase table names it for the next release. [The window](manual/02-the-window.md) describes the Inspector's PLT colors and their palettes; [the command line](manual/05-command-line.md) describes `--plt-colors`.

## 2026-10-04
* **Update**: Made `docs/` an Open Knowledge Format (OKF v0.2) bundle: frontmatter on [the plan](PLAN.md) and on every chapter of [the manual](manual/README.md), index files and this log. No content changed. The program's manual reader (`crates/mgv-ui/src/manual.rs`) now skips a chapter's frontmatter.
