# Update log

## 2026-10-07
* **Update**: Before the repository went public: [the plan](PLAN.md) and the packaging notes no longer describe a private toolset repository (it is public; no token is needed), and nwnmdlcomp is looked for only in `NWN_TOOLS_BIN` and on `PATH` ([editing models](manual/03-editing-models.md)). The README has pictures, in `docs/images/`.
* **Update**: [The plan](PLAN.md) §10 records the move to Moonglow Toolset 1.16.1 (its renderer changes, and the window's labels and headings styled as the toolset's) and the phase table names it for the next release. [The window](manual/02-the-window.md) describes the Inspector's PLT colors and their palettes; [the command line](manual/05-command-line.md) describes `--plt-colors`.

## 2026-10-04
* **Update**: Made `docs/` an Open Knowledge Format (OKF v0.2) bundle: frontmatter on [the plan](PLAN.md) and on every chapter of [the manual](manual/README.md), index files and this log. No content changed. The program's manual reader (`crates/mgv-ui/src/manual.rs`) now skips a chapter's frontmatter.
