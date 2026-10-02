//! Where the stage's textures come from: the toolset's lookup
//! (`mg_render::Assets` for the resource manager), except that an object
//! with PLT colours (a creature's or an item's part) takes the PLT of a
//! name before a TGA or DDS of the same name, as the game does: a dwarf's
//! head names `pmd0_head001`, which is both a PLT and a plain gray TGA, and
//! the game colours it with the creature's skin. (Proposed upstream:
//! `docs/PLAN.md` §10.)

use mg_core::{ResRef, ResType};
use mg_image::Texture;
use mg_image::plt::{PALETTES, Plt};
use mg_image::txi::Txi;
use mg_render::{Assets, LoadedTexture, split_colors};
use mg_resman::{ResKey, ResMan};

/// The stage's textures from a resource manager.
#[derive(Debug, Clone, Copy)]
pub struct Textures<'a>(pub &'a ResMan);

impl Assets for Textures<'_> {
    fn texture(&self, name: &str) -> Option<LoadedTexture> {
        let rm = self.0;
        if let (base, Some(colors)) = split_colors(name)
            && let Ok(r) = ResRef::from_str(base)
            && !rm.contains(&ResKey::new(r, ResType::MTR))
            && let Ok(data) = rm.get(&ResKey::new(r, ResType::PLT))
            && let Ok(plt) = Plt::read(&data)
            && let Some(texture) = colored(rm, &plt, colors)
        {
            let txi =
                rm.get(&ResKey::new(r, ResType::TXI)).map(|d| Txi::parse(&d)).unwrap_or_default();
            return Some(LoadedTexture { texture, txi, mtr: None });
        }
        Assets::texture(rm, name)
    }

    fn material(&self, name: &str) -> Option<mg_image::mtr::Mtr> {
        Assets::material(self.0, name)
    }

    fn txi(&self, name: &str) -> Option<Txi> {
        Assets::txi(self.0, name)
    }
}

/// A PLT coloured with a colour per layer, from the game's palettes.
fn colored(rm: &ResMan, plt: &Plt, colors: [u8; 10]) -> Option<Texture> {
    let palettes: Vec<Option<mg_image::Rgba>> = PALETTES
        .iter()
        .map(|p| {
            let (t, data) = rm.texture(ResRef::from_str(p).ok()?)?;
            mg_image::read(t, &data).ok().map(|t| t.to_rgba())
        })
        .collect();
    let refs: [Option<&mg_image::Rgba>; 10] = std::array::from_fn(|i| palettes[i].as_ref());
    Some(plt.colorize(&refs, colors).into_texture(true))
}
