# The window

The window is made of panels you can drag into other places (**View ›
Reset Layout** puts them back):

- **3D View**: what is open. Drag with the left button to turn around it,
  with the right or middle button to move, and turn the wheel to come
  closer. Double-click (or F) frames it. **View** jumps to the front,
  back, sides, top or bottom of what is open: creatures face +Y, the
  game's placeables −Y (an armoire's doors, a chair's seat), and the
  camera keeps its angle to the front from one model to the next. **Grid** shows the ground (1 m squares,
  10 m across, a tile's size), **Axes** shows X (red), Y (green, where
  models face) and Z (blue, up). **Overlays** adds the walkmesh (faces by
  surface material: green where creatures walk, red where they do not),
  the wireframe, the normals and the node tree. What the model hides shows
  faintly; the node tree and the selected node's box show over everything.
  Click a part of the model to select its node.
- **ASCII**: the model's text (see [Editing models](03-editing-models.md)).
- **Texture**: textures you open.
- **Resources**: the game's resources (see [Getting started](01-getting-started.md)).
- **Nodes**: the model's node tree; click to select. Parts of a creature
  (head, limbs, equipment, wings) are listed under it.
- **Inspector**: what is open and the selected node: its type, position,
  orientation, controllers, mesh, textures (and whether the game has
  them), emitter or light settings.
- **Animation**: the animations the model can play (its own and its
  supermodels'), play and pause, loop or once, speed, and a slider through
  the animation with its events marked.
- **Effects** and **Lighting**: see [Lighting and visual effects](04-lighting-and-effects.md).
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
| Ctrl+Q | Quit |
