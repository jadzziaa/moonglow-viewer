# Getting started

## What you need

Moonglow Viewer reads the game's files from your installation of
Neverwinter Nights: Enhanced Edition. It finds the game where Steam
installs it; if yours is elsewhere, choose **File › Game Folder…** (the
folder with `data/nwn_base.key` in it), or set `NWN_ROOT` for the command
line. Without the game you can still open model files with their textures
beside them.

The 3D view needs a graphics card with Vulkan, Metal, DirectX 12 or
OpenGL 3.3.

## Opening things

- **File › Open…** (Ctrl+O), or drop a file on the window, or name it on
  the command line: `moonglow-viewer path/to/model.mdl`.
- **Resources**: everything the game has, in the game's load order. Type
  part of a name, pick a kind (models, creatures, textures, blueprints,
  walkmeshes, materials) and double-click.
- **Creatures** in Resources lists `appearance.2da`: double-click a row to
  see that creature bare; the Inspector then changes its gender,
  phenotype, head, body parts, colors, wings and tail.

A file opened from disk brings its folder along: textures, materials and
supermodels beside it win over the game's, as an override folder would.
**File › Add Folder…** and **File › Add Hak, Module or ERF…** put more
folders and archives above the game; the viewer remembers them.

## What it opens

| File | Shown as |
| --- | --- |
| `.mdl` (compiled or ASCII) | The model, playing what the game plays for its kind (a creature's pause, a placeable's default, a visual effect's impact then duration) |
| `.wok`, `.pwk`, `.dwk` | The walkmesh |
| `.tga`, `.dds`, `.plt` | The texture, with its mip levels, channels and TXI; a PLT with its layers colored |
| `.utc`, `.uti`, `.utp`, `.utd` | The creature, item, placeable or door, assembled as the game shows it |

Nothing you open is changed on disk unless you save it.
