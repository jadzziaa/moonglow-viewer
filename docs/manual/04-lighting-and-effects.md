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
