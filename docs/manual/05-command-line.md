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
mgv info c_wererat                                        # JSON
mgv decompile c_golemerald -o ascii/
mgv compile ascii/c_golemerald.mdl --with nwnmdlcomp
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
  `--vfx ROW` (repeatable) visual effects. These apply to galleries too.
- **Galleries** (`gallery`): a name pattern (`plc_*`), a 2DA
  (`placeables`, `appearance`, `visualeffects`, `doors`), a hak, module or
  ERF, or a folder. Writes `images/`, `index.html` (a page to browse and
  search them) and `manifest.json` (each item's model, classification,
  nodes, faces, animations and bounds). Running it again redraws only what
  changed (the model, its supermodels or textures, or the picture's
  settings); `--full` redraws everything.
- **Everywhere**: `--root` (the game folder), `--hak` and `--folder`
  (more resources), `--no-user-dir` (leave your override out), `--no-game`.
