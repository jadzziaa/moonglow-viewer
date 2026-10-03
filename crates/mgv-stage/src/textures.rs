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

/// Whether a texture name resolves to anything (an MTR, a DDS or TGA, a
/// PLT).
fn exists(rm: &ResMan, name: &str) -> bool {
    let Ok(r) = ResRef::from_str(name) else { return false };
    rm.texture(r).is_some()
        || [ResType::PLT, ResType::MTR].iter().any(|t| rm.contains(&ResKey::new(r, *t)))
}

/// The race, gender and phenotype prefix and the rest of a body part
/// model's name (`pfh0_belt063` → `f`, `h`, `belt063`): `p`, m or f, a race
/// letter, a phenotype digit, `_`, the part.
fn part_name(model: &str) -> Option<(char, char, &str)> {
    let (prefix, rest) = model.split_once('_')?;
    let mut c = prefix.chars();
    match (c.next()?, c.next()?, c.next()?, c.next()?, c.next()) {
        ('p', g @ ('m' | 'f'), r, pheno, None) if pheno.is_ascii_digit() && !rest.is_empty() => {
            Some((g, r, rest))
        }
        _ => None,
    }
}

/// Textures for a body part model's meshes whose own texture names resolve
/// to nothing (lower case, from → to): the game then applies the texture of
/// the part's own name, else its race's at phenotype 0, else the human's of
/// its gender, else the human male's (nwn.wiki, PLT: "the game will
/// automatically apply pmy0_footl001.plt … it will default back to
/// pmh0_footl001.plt"). 60 part meshes in the base game name a texture
/// that does not exist beside one of their own name (`pfh0_belt063` names
/// `beltmerged`). A name that resolves is kept: whether the game would
/// replace it too is not known.
pub fn part_fallbacks(
    rm: &ResMan,
    model: &mg_mdl::Model,
    name: &str,
) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    let name = name.to_ascii_lowercase();
    let Some((g, r, rest)) = part_name(&name) else { return out };
    let candidates = [
        name.clone(),
        format!("p{g}{r}0_{rest}"),
        format!("p{g}h0_{rest}"),
        format!("pmh0_{rest}"),
    ];
    let mut own: Option<Option<&String>> = None;
    for n in &model.nodes {
        let Some(t) = n.mesh().filter(|m| m.render).and_then(|m| m.textures[0].as_ref()) else {
            continue;
        };
        if out.contains_key(t) || exists(rm, t) {
            continue;
        }
        let found = *own.get_or_insert_with(|| candidates.iter().find(|c| exists(rm, c)));
        if let Some(to) = found {
            out.insert(t.clone(), to.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_names() {
        assert_eq!(part_name("pfh0_belt063"), Some(('f', 'h', "belt063")));
        assert_eq!(part_name("pmd2_cloak_003"), Some(('m', 'd', "cloak_003")));
        assert_eq!(part_name("plc_a01"), None);
        assert_eq!(part_name("pmh0"), None);
        assert_eq!(part_name("c_badger"), None);
    }
}
