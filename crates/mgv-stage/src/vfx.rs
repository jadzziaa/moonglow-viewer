//! Visual effects: `visualeffects.2da` rows attached to an actor's hooks,
//! playing as the game plays them (the plan's §4.4).
//!
//! A row names a model per hook: the head (`Imp_HeadCon_Node`), the impact
//! point (`Imp_Impact_Node`) and the ground (`Imp_Root_{S,M,L,H}_Node`, by
//! the target's size). Each model plays `impact`, then loops `duration`, and
//! plays `cessation` when the effect is removed. Hooks are found by the
//! target's kind (nwn.wiki, Model Special Nodes): creatures `head`,
//! `impact`, their root; placeables `<name without its first 4
//! characters>_head_hit`, `_impact`, `_ground`; doors `hhit`, `impc`,
//! `grnd`.

use mg_mdl::{Classification, Model};
use mgv_library::Library;

use crate::subject::default_sequence;
use crate::{ActorId, Animations, Placement, PlayMode, Stage, StageError};

/// A row of `visualeffects.2da`.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualEffect {
    pub row: usize,
    pub label: String,
    /// F (instant), D (duration), P (projectile), B (beam).
    pub kind: char,
    /// Turns with the hook (not just placed at it).
    pub orient_with_object: bool,
    pub head: Option<String>,
    pub impact: Option<String>,
    /// Ground models for small, medium, large and huge targets.
    pub root: [Option<String>; 4],
    /// `progfx.2da` rows (impact, duration, cessation).
    pub progfx: [Option<u32>; 3],
}

impl VisualEffect {
    /// Whether it has any model to show.
    pub fn has_model(&self) -> bool {
        self.head.is_some() || self.impact.is_some() || self.root.iter().any(Option::is_some)
    }

    /// The ground model for a size (1–2 small, 3 medium, 4 large, 5
    /// huge). Only that size's column counts: the table leaves sizes empty
    /// on purpose (greater stoneskin has ground models for large and huge
    /// targets only; its look on others is a programmed skin effect).
    pub fn root_for(&self, size: u32) -> Option<&str> {
        let i = match size {
            0..=2 => 0,
            3 => 1,
            4 => 2,
            _ => 3,
        };
        self.root[i].as_deref()
    }
}

fn model_cell(t: &mg_2da::TwoDa, row: usize, col: &str) -> Option<String> {
    t.get(row, col).filter(|v| !v.is_empty() && *v != "****").map(str::to_ascii_lowercase)
}

/// Every row of `visualeffects.2da` with a model.
pub fn table(lib: &Library) -> Vec<VisualEffect> {
    let Ok(t) = lib.game().table("visualeffects") else { return Vec::new() };
    (0..t.len()).filter_map(|row| row_of(&t, row)).filter(VisualEffect::has_model).collect()
}

/// One row.
pub fn row(lib: &Library, row: usize) -> Option<VisualEffect> {
    let t = lib.game().table("visualeffects").ok()?;
    row_of(&t, row)
}

fn row_of(t: &mg_2da::TwoDa, row: usize) -> Option<VisualEffect> {
    let label = t.get(row, "Label").filter(|v| *v != "****")?.to_string();
    let int =
        |c: &str| t.get(row, c).and_then(mg_2da::parse_int).and_then(|v| u32::try_from(v).ok());
    Some(VisualEffect {
        row,
        label,
        kind: t.get(row, "Type_FD").and_then(|v| v.chars().next()).unwrap_or('F'),
        orient_with_object: int("OrientWithObject").is_some_and(|v| v != 0),
        head: model_cell(t, row, "Imp_HeadCon_Node"),
        impact: model_cell(t, row, "Imp_Impact_Node"),
        root: ["S", "M", "L", "H"].map(|s| model_cell(t, row, &format!("Imp_Root_{s}_Node"))),
        progfx: [int("ProgFX_Impact"), int("ProgFX_Duration"), int("ProgFX_Cessation")],
    })
}

/// Which hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hook {
    Head,
    Impact,
    Root,
}

/// The node a hook is on a model, by the model's kind; `None`: its root.
pub fn hook_node(model: &Model, hook: Hook) -> Option<usize> {
    let short = model.name.get(4..).unwrap_or("").to_ascii_lowercase();
    let names: Vec<String> = match (model.classification, hook) {
        (Classification::Door, Hook::Head) => vec!["hhit".into()],
        (Classification::Door, Hook::Impact) => vec!["impc".into()],
        (Classification::Door, Hook::Root) => vec!["grnd".into()],
        (_, Hook::Head) => {
            vec!["head".into(), format!("{short}_head_hit"), "headconjure".into(), "head_g".into()]
        }
        (_, Hook::Impact) => vec!["impact".into(), format!("{short}_impact")],
        (_, Hook::Root) => vec![format!("{short}_ground"), "root".into()],
    };
    let lower: Vec<String> = model.nodes.iter().map(|n| n.name.to_ascii_lowercase()).collect();
    names.iter().find_map(|want| lower.iter().position(|n| n == want || n.ends_with(want.as_str())))
}

/// What applying an effect added.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    pub effect: VisualEffect,
    pub actors: Vec<ActorId>,
    /// Models the row names that could not be loaded.
    pub missing: Vec<String>,
}

/// Attaches an effect's models to a target actor and starts them.
/// `size`: the target's `appearance.2da` `SIZECATEGORY` (3 for objects).
pub fn apply(
    stage: &mut Stage,
    lib: &Library,
    target: ActorId,
    effect: &VisualEffect,
    size: u32,
) -> Result<Applied, StageError> {
    let target_model = stage
        .actor(target)
        .map(|a| a.model.model.clone())
        .ok_or_else(|| StageError::NotShown("no target".into()))?;
    let mut actors = Vec::new();
    let mut missing = Vec::new();
    let hooks = [
        (effect.head.as_deref(), Hook::Head),
        (effect.impact.as_deref(), Hook::Impact),
        (effect.root_for(size), Hook::Root),
    ];
    for (model, hook) in hooks {
        let Some(name) = model else { continue };
        let Some(m) = lib.model(name) else {
            missing.push(name.to_string());
            continue;
        };
        let node = hook_node(&target_model, hook);
        let placement = Placement::On {
            parent: target,
            node,
            scale: 1.0,
            // Ground effects stay level; head and impact effects turn with
            // their hook where the row says so.
            follow: effect.orient_with_object && hook != Hook::Root,
        };
        let mut actor = stage.actor_for(lib, name, m.clone(), placement);
        let seq = default_sequence(&m, &actor.animations);
        let names: Vec<&str> = seq.iter().map(String::as_str).collect();
        actor.player.sequence(&names);
        actors.push(stage.add(actor));
    }
    Ok(Applied { effect: effect.clone(), actors, missing })
}

/// Ends an effect: its models play `cessation` once where they have one,
/// and the others disappear.
pub fn remove(stage: &mut Stage, applied: &Applied) {
    for &id in &applied.actors {
        let Some(a) = stage.actor_mut(id) else { continue };
        if a.animations.find("cessation").is_some() {
            a.player.play(Some("cessation"), PlayMode::Once);
        } else {
            a.visible = false;
        }
    }
}

/// The size category of a creature's appearance row (3 when unknown).
pub fn size_of(lib: &Library, appearance: usize) -> u32 {
    lib.game()
        .table("appearance")
        .ok()
        .and_then(|t| t.get(appearance, "SIZECATEGORY").and_then(mg_2da::parse_int))
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(3)
}

/// Whether an actor's animations include an effect sequence (for tests and
/// the inspector).
pub fn is_effect(animations: &Animations) -> bool {
    ["impact", "duration", "cessation"].iter().any(|n| animations.find(n).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_take_their_own_column() {
        let e = VisualEffect {
            row: 2,
            label: "VFX_DUR_ENTANGLE".into(),
            kind: 'D',
            orient_with_object: false,
            head: None,
            impact: None,
            root: [None, Some("m".into()), Some("l".into()), Some("h".into())],
            progfx: [None; 3],
        };
        assert_eq!(e.root_for(1), None, "no small model: none");
        assert_eq!(e.root_for(3), Some("m"));
        assert_eq!(e.root_for(4), Some("l"));
        assert_eq!(e.root_for(5), Some("h"));
        assert!(e.has_model());
    }

    #[test]
    fn hooks_by_kind() {
        let node = |n: &str| mg_mdl::Node::new(n, mg_mdl::NodeKind::Dummy);
        let creature = Model {
            name: "c_rat".into(),
            classification: Classification::Character,
            nodes: vec![node("c_rat"), node("Head"), node("impact")],
            ..Default::default()
        };
        assert_eq!(hook_node(&creature, Hook::Head), Some(1));
        assert_eq!(hook_node(&creature, Hook::Impact), Some(2));
        assert_eq!(hook_node(&creature, Hook::Root), None);
        let placeable = Model {
            name: "plc_a01".into(),
            classification: Classification::Character,
            nodes: vec![
                node("plc_a01"),
                node("a01_head_hit"),
                node("a01_impact"),
                node("a01_ground"),
            ],
            ..Default::default()
        };
        assert_eq!(hook_node(&placeable, Hook::Head), Some(1));
        assert_eq!(hook_node(&placeable, Hook::Root), Some(3));
        let door = Model {
            name: "t_door09".into(),
            classification: Classification::Door,
            nodes: vec![node("t_door09"), node("t_door09hhit"), node("t_door09impc")],
            ..Default::default()
        };
        assert_eq!(hook_node(&door, Hook::Head), Some(1), "door hooks end with their name");
        assert_eq!(hook_node(&door, Hook::Impact), Some(2));
    }
}
