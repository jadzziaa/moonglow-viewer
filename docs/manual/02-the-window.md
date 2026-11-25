---
type: Manual Page
title: The window
description: The window's shell (Top Bar, Left, Right and Bottom Docks, editor area, Status Bar), how it gives way in small and wide windows, its panels and the keys.
tags: [manual, panels, keys]
generated: { by: claude-code/claude-opus-5-5, at: 2026-11-25T14:27:06Z }
---

# The window

The window is Moonglow's shell, as Moonglow Toolset's is:

- The **Top Bar**: the menus File, Model, View and Help, then **Open** and
  **Save** as buttons, and at its right end **Decompile**, **Compile** and
  **Compile and View** (each does what its menu item does, and is grayed
  out where the menu item is; in a window under 1300 points wide they
  show their icons alone).
- The **Left Dock**: **Resources** over the **Outliner**. The **Right
  Dock**: the **Inspector** over **Lighting**. Drag the line between two
  panels, or a dock's edge, to give one more room. The arrow at the end of
  a dock's first header folds the dock to a strip of its panels' icons;
  click the strip's arrow or an icon to bring it back.
- The **Bottom Dock**, under the editor area: **Timeline**, **Effects**
  and **Log** as tabs.
- The **editor area**: the **3D View** in one group, **Texture** and
  **ASCII** as tabs of a second beside it. Drag a tab to put it
  elsewhere in the area; close Texture or ASCII with the × on its tab
  (the View menu brings it back beside the other). The ASCII tab shows a
  dot while it has edits not saved.
- The **Status Bar**: at the left, a switch for each Bottom Dock panel
  (click the one in front to close the dock, another to bring it to the
  front) and, while the Log is out of sight, its last line (click it to
  show the Log); in the middle, what the mouse and the keys do in the
  panel under the pointer, as key caps; at the right, the document, where
  it was read from and the game folder.

The window opens 1440 by 900 points the first time, then at the size you
left it. The shell gives way in a smaller window: under 1500 points wide the Left
Dock folds to its strip, under 1400 the Right Dock is 280 points wide with
its panels as tabs and the game folder leaves the Status Bar, under 796
the Right Dock folds too (unfold either by hand when you need it). In a
window 2560 points wide or more (3440 by 1440, say) the Right Dock stands
in two columns, the Inspector over the Log and Lighting over Effects,
the Bottom Dock holds the Timeline alone, the editor area opens with the
3D View, Texture and ASCII in three groups side by side, and the 3D
View's toggles carry their words. **View › Reset Layout** puts every dock,
panel and tab back. **View › Theme** chooses the dark or the light look;
it is kept for the next run.

The panels:

- **3D View**: what is open. Its header holds, at the left, **Frame**
  and the **View** menu; at the right, **Grid** and **Axes**, the
  **Overlays** popover, **Shaders**, **Shadow** and **Model lights**
  (icons: hover one for its name) and the background's color swatch.
  Where the view is narrow, what does not fit goes under **More** (⋯) at
  the header's end. Drag with the left button to turn around it,
  with the right or middle button to move, and turn the wheel to come
  closer; W, A, S and D move it along the ground, Q and E down and up. Double-click (or F) frames it. **View** jumps to the front,
  back, sides, top or bottom of what is open: creatures face +Y, the
  game's placeables −Y (an armoire's doors, a chair's seat), and the
  camera keeps its angle to the front from one model to the next. **Grid** shows the ground (1 m squares,
  10 m across, a tile's size), **Axes** shows X (red), Y (green;
  creatures face +Y, placeables −Y) and Z (blue, up); both hide behind
  models. **Overlays** (a popover; in a window 2560 wide, toggles on the
  header itself) adds the walkmesh (faces by
  surface material: green where creatures walk, red where they do not),
  the wireframe, the normals and the node tree. Of these, what the model
  hides shows faintly; the node tree and the selected node's box show over everything.
  **Shadow casters**, also under Overlays, draws the meshes that cast
  shadows without being drawn (`render 0`, `shadow 1`) in blue in place
  of the visible meshes.
  **Shaders** (also **View › Game shaders**) draws with the game's own
  shaders, compiled from your installation and the haks you added: a
  hak's replacements of the game's (a different lighting), and the custom
  shaders a material names (`customshaderVS`/`FS`: swaying grass and
  leaves, flowing water), which move while the model plays. It is on
  unless you turn it off. The first time a shader is needed it is
  compiled while you look, Moonglow's own shader drawing until it is
  ready; compiled shaders are kept for the next time in the viewer's
  cache folder (`~/.cache/moonglow-viewer/shaders`, `%LOCALAPPDATA%\Moonglow
  Viewer\shaders`, `~/Library/Caches/Moonglow Viewer/shaders`). One that
  does not compile is drawn with Moonglow's own shader, and the log says
  which and why. Off, everything is drawn with Moonglow's own shader,
  which is matched to the game's.
  **Shadow** (also **View › Shadow**) lets the model cast the light's
  shadow on a ground under it, as an area's ground takes a creature's
  shadow; the ground itself is unseen, in the background's color, a
  centimeter under the model's lowest point where it stood when it was
  opened. It follows the lighting's sun or moon (Lighting tab). It is on
  unless you turn it off. Pictures from `mgv` draw no shadow, as the
  toolset's thumbnails and galleries draw none.
  **Model lights** lights the scene with the model's own light nodes; the
  swatch at the header's end picks the background's color.
  Click a part of the model to select its node.
- **ASCII**: the model's text (see [Editing models](03-editing-models.md)).
- **Texture**: textures you open (Ctrl and the wheel zoom it).
- **Resources**: the game's resources (see [Getting started](01-getting-started.md)).
- **Outliner**: the model's node tree; click to select. Parts of a creature
  (head, limbs, equipment, wings) are listed under it.
- **Inspector**: what is open and the selected node: its type, position,
  orientation, controllers, mesh, textures (and whether the game has
  them), emitter or light settings.
  For a model opened on its own that wears PLT textures (a body part, an
  animation base such as `a_halforc`), it has the ten **PLT colors** a
  creature or an item would give them: skin, hair, metal, cloth, leather
  and tattoos, each picked from its palette's 176 colors (the list
  beside its swatch shows them, as in Moonglow Toolset). **Reset colors** goes
  back to the model as opened.
- **Timeline**: the animations the model can play (its own and its
  supermodels'), play and pause, loop or once, speed, and a slider through
  the animation with its events marked.
- **Effects** (double-click an effect to apply it) and **Lighting**: see [Lighting and visual effects](04-lighting-and-effects.md).
- **Log**: what happened, warnings and errors.

## Keys

| Key | Does |
| --- | --- |
| Ctrl+O | Open |
| Ctrl+S, Ctrl+Shift+S | Save, Save As (the ASCII) |
| Ctrl+D | Decompile |
| Ctrl+B, Ctrl+Shift+B | Compile, Compile and View |
| F5 | Re-read everything from disk |
| F | Frame (in the 3D view) |
| W A S D, Q E | Move the camera along the ground where it looks, to the sides, down and up (in the 3D view; Shift: faster) |
| ↑ ↓, Page Up/Down, Home, End | Step through Resources, opening each (after a click in it) |
| Ctrl+Q | Quit |
