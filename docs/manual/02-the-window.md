---
type: Manual Page
title: The window
description: The window's shell (Top Bar, Left, Right and Bottom Docks, editor area, Status Bar), how it gives way in small and wide windows, its panels and the keys.
tags: [manual, panels, keys]
generated: { by: claude-code/claude-opus-5-5, at: 2026-11-25T16:18:39Z }
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
- Any of these panels moves to another dock: drag it by its header (a
  Bottom Dock panel by its tab) and drop it on the Left, Right or Bottom
  Dock, which is outlined while the pointer is over it (a folded, closed
  or empty dock takes the drop along its edge of the editor area; Esc
  lets go without moving it). Right-click a header or tab for the same
  as a menu: **Move to the Left Dock**, **Move to the Right Dock** (its
  first or second column in a wide window), **Move to the Bottom Dock**
  and **Reset Layout**.
- The **Bottom Dock**, under the editor area: **Timeline**, **Effects**
  and **Log** as tabs.
- The **editor area**: the **3D View** in one group, **Texture** and
  **ASCII** as tabs of a second beside it. Drag a tab to put it
  elsewhere in the area; close Texture or ASCII with the × on its tab
  (the View menu brings it back beside the other). The ASCII tab shows a
  dot while it has edits not saved. The **Gallery** tab (Resources'
  **Open as Tab**, or **View › Gallery**) stands in a group of its own
  left of the 3D View.
- The **Status Bar**: at the left, a switch for each Bottom Dock panel
  (click the one in front to close the dock, another to bring it to the
  front) and, while the Log is out of sight, its last line (click it to
  show the Log); in the middle, what the mouse and the keys do in the
  panel under the pointer, as key caps (over a gallery: Click Show,
  Up Down Row of pictures, Left Right One picture); at the right, the document, where
  it was read from and the game folder.

The window opens 1440 by 900 points the first time, then at the size you
left it, and with the layout you left: which dock each panel stands in,
the docks' widths and heights and the lines between their panels, which
dock is folded and which panel is in front, and the editor area's groups
and tabs. A layout from an older version opens as the default. The shell gives way in a smaller window: under 1500 points wide the Left
Dock folds to its strip, under 1400 the Right Dock is 280 points wide with
its panels as tabs and the game folder leaves the Status Bar, under 796
the Right Dock folds too (unfold either by hand when you need it; a fold
you make yourself is kept, the window's own are not). A layout kept from a
wider window gives way the same way. In a
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
- **Texture**: textures you open. Its header holds the channels to show
  (RGBA, RGB, R, G, B, A) at the left and **Mip** and **Zoom** at the right (a
  slider and a number each; Mip is grayed out where the texture has no
  mip levels);
  the caption line under it gives the texture's name, format, size, mip
  levels and alpha. Ctrl and the wheel zoom it.
- **Resources**: the game's resources (see [Getting started](01-getting-started.md)),
  as a **List** or a **Gallery** of pictures: the two buttons beside
  the count switch between them, and the choice is kept for the next
  run. In the gallery each model, blueprint, material (a sphere wearing
  it) and creature is pictured as `mgv gallery` pictures it (the same
  angle, framing and light, the game's shaders), a texture by the texture
  itself; walkmeshes and TXI show their kind's icon. The type is written
  under each name where the filter lists more than one, a creature's
  `appearance.2da` row under its label. The slider beside the buttons
  sets the pictures' size (the Gallery tab's slider is the same one). As in
  the list, a click shows the picture's model, texture or creature, and
  the arrow keys step and show each in turn (↑ ↓ a row, ← → one picture;
  Home, End, Page Up and Page Down too); a double-click or Enter opens it
  too. Pictures are made a few at a time, those in
  sight first, the kind's icon standing in until each is ready; one that
  cannot be made, or a model with nothing to draw (one whose meshes are
  all hidden, as the base game's `a_ba_coat`), says **No picture** (hover
  it to see why). They are made
  again after **File › Refresh** (F5) or when a folder or hak changes. In
  Gallery mode the Left Dock opens wider (three pictures a row).
  **Open as Tab** (the icon at the right of the Resources header) puts the
  gallery in the editor area as the **Gallery** tab, with the filter,
  the search, the size slider and the count in its header, and Resources
  goes back to its list.
- **Outliner**: the model's node tree; click to select. Parts of a creature
  (head, limbs, equipment, wings) are listed under it.
- **Inspector**: what is open and the selected node: its type, position,
  orientation, controllers, mesh, textures (and whether the game has
  them), emitter or light settings, each in a section of its own, and
  the shown model's name, classification, supermodel, nodes, faces and
  animations under **Model**.
  For a creature (from Resources' **Creatures**) the **Creature** section
  sets its **Gender** across the top, then **Phenotype**, **Skin color**,
  **Hair color**, **Tattoo 1**, **Tattoo 2**, **Head**, **Body parts**,
  **Wings** and **Tail**, two to a row where the dock is wide enough (one
  in a narrow one). A color is its swatch and a number beside it: click
  either to open the palette's 176 colors, sixteen to a row, and click
  one to take it.
  For a model opened on its own that wears PLT textures (a body part, an
  animation base such as `a_halforc`), it has the ten **PLT colors** a
  creature or an item would give them: skin, hair, metal, cloth, leather
  and tattoos, two to a row, each picked from its palette's 176 colors
  as a creature's are. **Reset Colors** goes back to the model as opened.
- **Timeline**: one row with the **Animation** to play (its own and its
  supermodels', chosen from a list), **Pause** or **Play**, **Restart**,
  **Loop** or **Once**, and the **Speed** (a slider where there is room,
  and a number); under it the scrub bar across the panel's width (click
  or drag to go to a time; the number after it says the time and moves
  it too, beside the animation's length), the animation's events marked
  on it and a ruler of seconds under it.
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
| ↑ ↓ ← →, Page Up/Down, Home, End | Step through a gallery, showing each (after a click in it) |
| Enter | Open the picture chosen in a gallery |
| Ctrl+Q | Quit |
