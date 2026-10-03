//! Textures for body part models opened on their own: the toolset's
//! creature previews give a part naming a missing texture the one of its
//! own name (`mg-preview`), and this does the same for a bare part model.

use mg_core::{ResRef, ResType};
use mg_resman::{ResKey, ResMan};

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
