---
type: Manual Page
title: The command line
description: The mgv command-line tool - pictures, turntables, galleries, model information, decompiling, compiling and linting without a window.
tags: [manual, command-line, mgv]
generated: { by: claude-code/claude-opus-5-5, at: 2026-10-03T08:07:12Z }
---

# The command line

`mgv` does without a window what the viewer does: pictures, galleries,
decompiling, compiling and checking. It needs a graphics card for
pictures, but no display. (In the AppImage: `./MoonglowViewer.AppImage mgv
…`, or through a link to the AppImage named `mgv`; on macOS: `"Moonglow
Viewer.app/Contents/MacOS/mgv"`.)

Inputs are files or the game's resources: `plc_a01`, `nw_chicken.utc`,
`appearance:6` (a creature by `appearance.2da` row).

```sh
mgv render plc_a01 -o chest.png --size 1024x1024 --time 1.5
mgv render appearance:6 --vfx 4 --light env:0:night -o warded.png
mgv turntable c_golemerald --frames 48 -o golem.png      # an animated PNG
mgv gallery placeables -o gallery/ --size 256x256
mgv gallery "c_*" -o creatures/
mgv sheet build/ plc_a01 "c_drg*" -o sheet.png --columns 5 # one picture, a tile each
mgv info c_wererat                                        # JSON
mgv decompile c_golemerald -o ascii/
mgv compile ascii/c_golemerald.mdl                        # in process: no game, no window
mgv compile ascii/plc_a01.mdl --with engine              # the game's own compiler
mgv lint mymodel.mdl --notes
```

- **Pictures** (`render`, `turntable`): `--anim` and `--time` choose the
  moment (emitters and animations run up to it, the same each time),
  `--view`, `--yaw`, `--pitch` and `--zoom` the camera (`--view` from
  the model's front: creatures face +Y, placeables −Y; `--yaw` in degrees
  from +X whatever the model, 90 from +Y, -90 from −Y), `--light` the
  lighting (`studio`, or `env:ROW` / `env:ROW:night`), `--key-light`
  a light at the camera (0 to 1; about 0.3 is as strong as the studio
  sun) so the sides in view are lit whatever the sun's direction, `--fog`,
  `--vfx ROW` (repeatable) visual effects, `--plt-colors` the colors of a
  model on its own that wears PLT textures (a body part, an animation
  base): `metal1=40,leather1=12` (layers skin, hair, metal1, metal2,
  cloth1, cloth2, leather1, leather2, tattoo1, tattoo2; palette rows 0 to
  175; the rest 0) or ten rows in that order. Creatures and blueprints
  keep their own colors. `--casters` shows a model's shadow casters (the
  meshes with `render 0` and `shadow 1`) in blue in place of its visible
  meshes. A picture is framed as close as shows all of the model from its
  angle; a turntable from far enough for every side. These apply to
  galleries and sheets too.
- **Galleries** (`gallery`): a name pattern (`plc_*`), a 2DA
  (`placeables`, `appearance`, `visualeffects`, `doors`), a hak, module or
  ERF, or a folder. Writes `images/`, `index.html` (a page to browse and
  search them) and `manifest.json` (each item's model, classification,
  nodes, faces, animations and bounds). Running it again redraws only what
  changed (the model, its supermodels or textures, or the picture's
  settings); `--full` redraws everything.
- **Contact sheets** (`sheet`): model files, folders of them, resource
  names, or anything a gallery takes, into one PNG: a tile each (`--size`,
  320x320 unless given) with its name and its width, depth and height in
  meters under it (`--no-labels` leaves them out), `--columns` to a row.
  What cannot be shown keeps its tile, with the reason in it.
- **Compiling** (`compile`): in process by the viewer's own compiler
  (`--with native`, the default): no game and no window, every kind of
  model, skin meshes with up to 64 bones included. It keeps the part
  numbers of the compiled model of the same name, and says what it left
  out by `file:line`. `--with engine` runs the game's compiler instead,
  `--with nwnmdlcomp` that program.
- **Checking** (`lint`): problems by `file:line`. It fails when it finds
  errors; `--strict` fails on warnings too, for build pipelines.
- **Everywhere**: `--root` (the game folder), `--hak` and `--folder`
  (more resources), `--no-user-dir` (leave your override out), `--no-game`.
