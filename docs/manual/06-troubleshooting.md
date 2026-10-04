---
type: Manual Page
title: Troubleshooting
description: Common problems (game not found, missing textures, differences from the game, compile failures, crashes) and what to do about them.
tags: [manual, troubleshooting]
generated: { by: claude-code/claude-opus-5-5, at: 2026-10-01T19:48:40Z }
---

# Troubleshooting

**The game is not found.** Choose **File › Game Folder…** (the folder with
`data/nwn_base.key`), or set `NWN_ROOT` for `mgv`.

**A texture is missing (white or pink).** The Inspector says, for the
selected mesh, whether the game has each texture. Textures beside a model
opened from disk are found; others need their folder or hak added (**File
› Add Folder…**).

**A model looks different in the game.** The viewer runs its own shaders,
matched to the game's enhanced lighting; hak shaders and MTR custom
shaders are not run. Dangly-mesh motion and some emitter details follow
the documentation rather than measurements.

**Compiling fails.** The log says which compiler ran and what it said.
The game's compiler cannot compile skin meshes (use nwnmdlcomp); nwnmdlcomp
refuses EE-only keywords and skins over 17 bones. If neither is found,
install neverwinter.nim's tools or Neverblender's, or put nwnmdlcomp on
`PATH`.

**Crashes.** A report is written to the viewer's data folder
(`~/.local/share/moonglow-viewer`, `%APPDATA%\Moonglow Viewer` or
`~/Library/Application Support/Moonglow Viewer`) as `crash-<time>.txt`.
