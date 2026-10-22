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

**A model looks different in the game.** The viewer draws with the game's
own shaders, from your installation and the haks you added, so add the
haks the module uses (a hak can replace the game's lighting). Where one
does not compile, the log names it and Moonglow's own shader draws
instead; with **Shaders** off, everything is Moonglow's (matched to the
game's enhanced lighting). Neither draws what reads the screen behind a
mesh (a refraction), and shiny water is Moonglow's. Dangly-mesh motion and
some emitter details follow the documentation rather than measurements.

**The view is slow the first time a model shows.** The game's shaders are
compiled the first time each is needed (Moonglow's shader draws until
they are) and kept for the next time; turn **Shaders** off to skip them.

**Compiling fails.** The log says which compiler ran and what it said.
The game's compiler cannot compile skin meshes (use nwnmdlcomp); nwnmdlcomp
refuses EE-only keywords and skins over 17 bones. If neither is found,
install neverwinter.nim's tools or Neverblender's, or put nwnmdlcomp on
`PATH`.

**Crashes.** A report is written to the viewer's data folder
(`~/.local/share/moonglow-viewer`, `%APPDATA%\Moonglow Viewer` or
`~/Library/Application Support/Moonglow Viewer`) as `crash-<time>.txt`.
