---
type: Manual Page
title: Lighting and visual effects
description: Lighting a model as the game does (studio, area presets, custom area) and applying the game's visual effects and emitters to it.
tags: [manual, lighting, visual-effects]
generated: { by: claude-code/claude-opus-5-5, at: 2026-10-02T00:52:54Z }
---

# Lighting and visual effects

## Lighting

The view lights models as the game's enhanced lighting does: the sun or
moon, the model's own lights (switch them off under **Lighting**), tile
lights, fog, environment maps and EE materials. **Lighting** chooses:

- **Studio**: a neutral daylight on a plain background (pick its color in
  the 3D view's toolbar). The default.
- **An area preset**: the area wizard's lighting schemes
  (`environment.2da`), by day or by night, with or without fog. Tiles'
  main lights take the scheme's colors.
- **Custom area**: the sun or moon's ambient and diffuse colors, the fog
  and the tile lights, as an area's properties hold them.

The sun and moon always come from the game's direction, behind a
placeable's front (placeables face −Y). Meshes whose material has no
ambient color, common in exported custom content, are black where the sun
doesn't reach, in the game as here. For pictures of such models the
command line can add a **key light** at the camera (`mgv render
--key-light 0.3`; see [The command line](05-command-line.md)).

## Visual effects

**Effects** lists the game's visual effects (`visualeffects.2da`).
Double-click one to apply it to what is shown: its models attach to the
head, the impact point or the ground, as the game attaches them (for
placeables and doors, at their own hook nodes), and play their impact then
their duration. **Remove** plays an effect's cessation; **Clear** takes all
off. Ground effects have a model per creature size: the target's own size
is used unless you choose another.

Emitters run in every model: fountains, explosions, point-to-point beams,
lightning and chunks. Restart them with **Animation › Restart**.
