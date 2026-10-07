---
type: Manual Page
title: Editing models
description: The ASCII editor with its live reload and problem list, decompiling compiled models, and compiling with the game's compiler or nwnmdlcomp.
tags: [manual, ascii, compiling, decompiling]
generated: { by: claude-code/claude-opus-5-5, at: 2026-10-01T20:45:29Z }
---

# Editing models

## The ASCII editor

An ASCII model opens in the **ASCII** panel beside the view. Each change
reloads the model in the view a moment after you stop typing, keeping the
camera, the animation and the selection; the file on disk is only written
when you save (Ctrl+S). The editor stays quick on the largest models (the
game's `a_ba` animation library is 90,000 lines decompiled).

- Keywords, node types, numbers and comments are colored.
- **Problems** are listed under the text and marked beside their lines:
  keywords the game does not know (it skips them), list counts that do not
  match, faces naming vertices that do not exist, parents that are not
  nodes above, danglymeshes without a constraint per vertex (the game
  crashes), names too long for the game, skins with more bones than a
  compiler takes. Click one to go to its line.
- The cursor and the view follow each other: put the cursor in a node's
  block and the node is selected; click a node in the view or the node
  tree and the editor goes to its block.
- If another program changes the file, the editor and the view take the new
  text; if you have unsaved edits, the editor asks which to keep.

Usual editing keys work: Shift with the arrows selects, Ctrl with the
arrows moves by words, Tab and Shift+Tab indent the selected lines,
Ctrl+Z and Ctrl+Y undo and redo, Ctrl+A selects everything.

## Decompiling

**Model › Decompile** (Ctrl+D) turns a compiled model into ASCII in the
editor; the view shows the ASCII at once, so you see it is the same model.
Save it to keep it. The viewer's own decompiler is exact: all 25,597
compiled models of the game decompile and read back the same, including
EE's normals. nwnmdlcomp can do it instead (**Model › Decompile with**).

The nodes are written in the order the game numbers them, which is not
always the tree's. The game finds a supermodel's animations for a model by
those numbers, and a compiler numbers nodes in the order of the text: so
**keep the nodes' order** when you edit a model other models use as their
supermodel, and add new nodes at the end. (A model with a supermodel of its
own, such as `a_ba`, cannot keep all its numbers through text; compiling
one again can break the creatures built on it.)

## Compiling

**Model › Compile** (Ctrl+B) compiles the editor's text into
`compiled/<name>.mdl` beside the ASCII file (asked for a folder when the
model is not a file yet). **Compile and View** then shows the compiled
model, as the game will load it. The compiler:

- **The game's compiler** (Automatic's choice for models without skin
  meshes) runs your game's own `nwmain` with the `compilemodel` command, in
  a scratch user folder of the viewer's (never your game's user folder). It
  keeps EE's materials and normals. On Linux it runs out of sight in
  gamescope's headless mode when gamescope is installed; elsewhere a game
  window opens for a moment. It cannot compile skin meshes.
- **nwnmdlcomp** (Automatic's choice for skin meshes) is found in the folder
  the `NWN_TOOLS_BIN` environment variable names, or on `PATH`. It
  predates EE: the viewer refuses models using `materialname` or
  `renderhint` (it would drop them) and skins with more than 17 bones.
