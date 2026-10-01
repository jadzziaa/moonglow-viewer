//! Putting what was opened on the stage: a model, a walkmesh, or a
//! blueprint assembled as the game shows it; and what it plays.

use std::sync::Arc;

use glam::Mat4;
use mg_core::{ResRef, ResType};
use mg_gff::{Gff, Struct, Value};
use mg_mdl::{Classification, Model};
use mg_resman::ResKey;
use mgv_library::{Kind, Library, Opened};

use crate::{ActorId, Animations, Placement, Stage, StageError, posed};

/// What [`show`] put on the stage.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub base: ActorId,
    pub parts: Vec<ActorId>,
    /// Models a blueprint names that could not be loaded.
    pub missing: Vec<String>,
}

/// The animations a model plays when shown on its own, in order (all but
/// the last once): what the game plays for its kind of model (the toolset's
/// `notes_models.md` C1).
pub fn default_sequence(model: &Model, anims: &Animations) -> Vec<String> {
    let has = |n: &str| anims.find(n).is_some();
    let first = |names: &[&str]| names.iter().find(|n| has(n)).map(|n| n.to_string());
    let seq: Vec<String> = match model.classification {
        Classification::Effect => {
            if has("impact") && has("duration") {
                vec!["impact".into(), "duration".into()]
            } else {
                first(&["duration", "impact", "conjure01", "cast01", "travel01", "default"])
                    .into_iter()
                    .collect()
            }
        }
        Classification::Character => {
            first(&["pause1", "cpause1", "default", "on", "open"]).into_iter().collect()
        }
        Classification::Tile => first(&["tiledefault", "day", "default"]).into_iter().collect(),
        Classification::Door => first(&["closed", "default"]).into_iter().collect(),
        Classification::Other => first(&["default", "pause1", "cpause1"]).into_iter().collect(),
    };
    seq
}

/// Starts an actor's default animations.
pub fn play_default(stage: &mut Stage, id: ActorId) {
    let Some(a) = stage.actor_mut(id) else { return };
    let seq = default_sequence(&a.model.model.clone(), &a.animations);
    let names: Vec<&str> = seq.iter().map(String::as_str).collect();
    a.player.sequence(&names);
}

/// Puts what was opened on the (cleared) stage and starts its default
/// animation.
pub fn show(stage: &mut Stage, lib: &Library, opened: &Opened) -> Result<Shown, StageError> {
    stage.clear();
    match opened.kind {
        Kind::Model => {
            let model = Model::read(&opened.data)
                .map_err(|e| StageError::Unreadable(format!("{}: {e}", opened.name())))?;
            let actor = stage.actor_for(
                lib,
                &opened.name(),
                Arc::new(model),
                Placement::World(Mat4::IDENTITY),
            );
            let base = stage.add(actor);
            play_default(stage, base);
            // Its walkmesh, for the overlay.
            if let Some((_, w)) = posed::walkmesh_for(lib, &opened.name()) {
                stage.walkmeshes.push(posed::Walkmesh {
                    name: opened.name(),
                    model: w,
                    host: Some(base),
                });
            }
            Ok(Shown { base, parts: Vec::new(), missing: Vec::new() })
        }
        Kind::Walkmesh => {
            // On the model it belongs to (same name), else alone.
            let walkmesh = Model::read(&opened.data)
                .map_err(|e| StageError::Unreadable(format!("{}: {e}", opened.name())))?;
            let model = lib.model(&opened.name()).unwrap_or_else(|| {
                // Nothing to draw: a bare root for the view to hold on to.
                Arc::new(Model {
                    name: opened.name(),
                    nodes: vec![mg_mdl::Node::new(&opened.name(), mg_mdl::NodeKind::Dummy)],
                    ..Default::default()
                })
            });
            let actor =
                stage.actor_for(lib, &opened.name(), model, Placement::World(Mat4::IDENTITY));
            let base = stage.add(actor);
            play_default(stage, base);
            stage.walkmeshes.push(posed::Walkmesh {
                name: opened.name(),
                model: Arc::new(walkmesh),
                host: Some(base),
            });
            Ok(Shown { base, parts: Vec::new(), missing: Vec::new() })
        }
        Kind::Blueprint => {
            let key = opened
                .key
                .ok_or_else(|| StageError::NotShown("a blueprint without a type".into()))?;
            let gff = Gff::read(&opened.data)
                .map_err(|e| StageError::Unreadable(format!("{}: {e}", opened.name())))?;
            let game = lib.game();
            let items =
                |r: ResRef| lib.get(&ResKey::new(r, ResType::UTI)).and_then(|d| Gff::read(&d).ok());
            let preview = match key.restype {
                ResType::UTC => mg_preview::creature(game, &gff.root, &items),
                ResType::UTI => mg_preview::item(game, &gff.root),
                ResType::UTP => mg_preview::placeable(game, &gff.root),
                ResType::UTD => mg_preview::door(game, &gff.root),
                t => return Err(StageError::NotShown(format!("{t:?} blueprints"))),
            }
            .map_err(|e| StageError::Unreadable(format!("{}: {e}", opened.name())))?;
            let added = stage.add_preview(lib, &preview, Mat4::IDENTITY)?;
            if preview.idle.is_none() {
                play_default(stage, added.base);
            }
            Ok(Shown { base: added.base, parts: added.parts, missing: added.missing })
        }
        Kind::Material => {
            // On a sphere: a mesh whose texture is the material's name finds
            // the MTR as the game does.
            let name = opened.name();
            let mtr = mg_image::mtr::Mtr::parse(&opened.data);
            let sphere = material_sphere(&name, &mtr);
            let actor =
                stage.actor_for(lib, &name, Arc::new(sphere), Placement::World(Mat4::IDENTITY));
            let base = stage.add(actor);
            Ok(Shown { base, parts: Vec::new(), missing: Vec::new() })
        }
        k => Err(StageError::NotShown(format!("{k:?}"))),
    }
}

/// A creature by `appearance.2da` row, without a blueprint: bare (part-based
/// bodies get body parts 1 and head `head`), default colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatureLook {
    pub appearance: u16,
    /// 0 male, 1 female.
    pub gender: u8,
    pub phenotype: u8,
    pub head: u16,
    /// PLT colour rows.
    pub skin: u8,
    pub hair: u8,
    pub tattoo1: u8,
    pub tattoo2: u8,
    /// `wingmodel.2da` and `tailmodel.2da` rows (0: none).
    pub wings: u32,
    pub tail: u32,
}

impl CreatureLook {
    pub fn new(appearance: u16) -> CreatureLook {
        CreatureLook {
            appearance,
            gender: 0,
            phenotype: 0,
            head: 1,
            skin: 0,
            hair: 0,
            tattoo1: 0,
            tattoo2: 0,
            wings: 0,
            tail: 0,
        }
    }

    /// The creature as a blueprint's fields.
    pub fn utc(&self) -> Struct {
        let mut utc = Struct::new(0);
        utc.set("Appearance_Type", Value::Word(self.appearance));
        utc.set("Gender", Value::Byte(self.gender));
        utc.set("Phenotype", Value::Int(i32::from(self.phenotype)));
        utc.set("Appearance_Head", Value::Byte(self.head.min(255) as u8));
        for f in [
            "BodyPart_RFoot",
            "BodyPart_LFoot",
            "BodyPart_RShin",
            "BodyPart_LShin",
            "BodyPart_LThigh",
            "BodyPart_RThigh",
            "BodyPart_Pelvis",
            "BodyPart_Torso",
            "BodyPart_Neck",
            "BodyPart_RFArm",
            "BodyPart_LFArm",
            "BodyPart_RBicep",
            "BodyPart_LBicep",
            "BodyPart_RHand",
            "BodyPart_LHand",
        ] {
            utc.set(f, Value::Byte(1));
        }
        for (f, v) in [
            ("Color_Skin", self.skin),
            ("Color_Hair", self.hair),
            ("Color_Tattoo1", self.tattoo1),
            ("Color_Tattoo2", self.tattoo2),
        ] {
            utc.set(f, Value::Byte(v));
        }
        utc.set("Wings_New", Value::Dword(self.wings));
        utc.set("Tail_New", Value::Dword(self.tail));
        utc
    }
}

/// What a creature of an appearance row looks like.
pub fn creature_preview(
    lib: &Library,
    look: &CreatureLook,
) -> Result<mg_preview::Preview, StageError> {
    mg_preview::creature(lib.game(), &look.utc(), &|_| None)
        .map_err(|e| StageError::Unreadable(format!("appearance {}: {e}", look.appearance)))
}

/// Puts a creature of an appearance row on the (cleared) stage.
pub fn show_creature(
    stage: &mut Stage,
    lib: &Library,
    look: &CreatureLook,
) -> Result<Shown, StageError> {
    stage.clear();
    let preview = creature_preview(lib, look)?;
    let added = stage.add_preview(lib, &preview, Mat4::IDENTITY)?;
    Ok(Shown { base: added.base, parts: added.parts, missing: added.missing })
}

/// A sphere of 1 m radius wearing a material (an MTR by name), for
/// previewing it: UVs wrap once around and pole to pole.
pub fn material_sphere(name: &str, mtr: &mg_image::mtr::Mtr) -> Model {
    use mg_mdl::{Face, Mesh, Node, NodeKind};
    const RINGS: usize = 24;
    const SEGMENTS: usize = 48;
    let mut mesh = Mesh {
        diffuse: [1.0; 3],
        ambient: [1.0; 3],
        specular: [0.0; 3],
        shininess: 1.0,
        render: true,
        shadow: true,
        textures: [Some(name.to_ascii_lowercase()), None, None, None],
        material: Some(name.to_ascii_lowercase()),
        renderhint: match mtr.renderhint {
            mg_image::mtr::RenderHint::None => None,
            mg_image::mtr::RenderHint::NormalAndSpecMapped => Some("normalandspecmapped".into()),
            mg_image::mtr::RenderHint::NormalTangents => Some("normaltangents".into()),
        },
        ..Default::default()
    };
    for r in 0..=RINGS {
        let v = r as f32 / RINGS as f32;
        let theta = v * std::f32::consts::PI;
        for s in 0..=SEGMENTS {
            let u = s as f32 / SEGMENTS as f32;
            let phi = u * std::f32::consts::TAU;
            let n = [theta.sin() * phi.cos(), theta.sin() * phi.sin(), theta.cos()];
            mesh.vertices.push([n[0], n[1], n[2] + 1.0]);
            mesh.normals.push(n);
            mesh.uvs[0].push([u, 1.0 - v]);
            let i = mesh.source.len() as u32;
            mesh.source.push(i);
            mesh.source_uv.push(i);
        }
    }
    let at = |r: usize, s: usize| (r * (SEGMENTS + 1) + s) as u32;
    for r in 0..RINGS {
        for s in 0..SEGMENTS {
            let (a, b, c, d) = (at(r, s), at(r + 1, s), at(r + 1, s + 1), at(r, s + 1));
            mesh.faces.push(Face { vertices: [a, b, c], material: 0 });
            mesh.faces.push(Face { vertices: [a, c, d], material: 0 });
        }
    }
    let mut root = Node::new(name, NodeKind::Dummy);
    root.children = vec![1];
    let mut sphere = Node::new("sphere", NodeKind::Mesh(Box::new(mesh)));
    sphere.parent = Some(0);
    Model {
        name: name.to_string(),
        classification: mg_mdl::Classification::Other,
        animation_scale: 1.0,
        nodes: vec![root, sphere],
        ..Default::default()
    }
}
