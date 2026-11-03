//! What the viewer shows, without a window.
//!
//! A [`Stage`] holds [`Actor`]s: model instances with their animation,
//! particles, dangly meshes and lights, each placed in the world or hanging
//! from another actor's node. A plain model is one actor; a blueprint is a
//! base actor with its parts (body parts, equipment, wings, tails); a
//! visual effect is actors hanging from its target's hooks. Unlike the
//! toolset's `Composed`, every actor runs its own particles and dangly
//! meshes.
//!
//! Time is explicit: [`Stage::step`] advances everything by `dt`, and
//! [`Stage::scene`] gives the renderer's scene for a camera. Windows feed
//! their frame time; headless renders step at fixed intervals, so the same
//! request gives the same image (particles use a fixed seed).

pub mod actor;
pub mod camera;
pub mod headless;
pub mod lighting;
pub mod overlay;
pub mod posed;
pub mod render;
pub mod subject;
pub mod textures;
pub mod vfx;

use std::collections::HashMap;
use std::sync::Arc;

use glam::{Mat4, Vec3};
use mg_mdl::Model;
use mg_preview::Preview;
use mg_render::{Gpu, GpuModel, Instance, MeshState, PointLight, Scene, anim};
use mgv_library::Library;
use thiserror::Error;

pub use actor::{Actor, ActorId, Animations, Look, Placement, PlayMode, Player};
pub use camera::OrbitCamera;
pub use lighting::Lighting;
pub use render::{Viewport, set_shader_cache, shader_cache_dir};

/// The game's own shaders draw unless switched off, as in Moonglow
/// Toolset (its `GAME_SHADERS_BY_DEFAULT`): compiled from the game's
/// files and the haks added, a hak's replacements of the stock ones and
/// the custom shaders materials name; Moonglow's own shader draws what
/// does not compile.
pub const GAME_SHADERS_BY_DEFAULT: bool = true;

use actor::Current;

#[derive(Debug, Error)]
pub enum StageError {
    #[error("model {0} not found or not readable")]
    NoModel(String),
    #[error("{0}")]
    Unreadable(String),
    #[error("the viewer does not show {0} in 3D")]
    NotShown(String),
}

/// What [`Stage::add_preview`] added.
#[derive(Debug, Clone, PartialEq)]
pub struct Added {
    /// The base actor; the parts follow it.
    pub base: ActorId,
    pub parts: Vec<ActorId>,
    /// Models the preview names that could not be loaded.
    pub missing: Vec<String>,
}

/// Everything shown, and its lighting.
#[derive(Debug)]
pub struct Stage {
    gpu: Gpu,
    actors: Vec<Actor>,
    /// Chunk emitters' models, by name.
    chunk_models: HashMap<String, Option<Arc<GpuModel>>>,
    /// Chunk particles from the last step.
    chunks: Vec<Instance>,
    pub lighting: Lighting,
    /// Light the scene with the models' own light nodes.
    pub model_lights: bool,
    /// Seconds stepped since the stage was filled.
    pub elapsed: f32,
    /// Walkmeshes shown with the models (overlays only; the renderer does
    /// not draw them).
    pub walkmeshes: Vec<posed::Walkmesh>,
    /// Which way what is shown faces (yaw, radians around Z from +X), for
    /// the camera's views ([`subject::front`]).
    pub front: f32,
    /// Draw the models' shadow casters (`render 0`, `shadow 1`) in blue in
    /// place of their visible meshes ([`actor::casters_view`]).
    pub casters: bool,
    /// The models cast the area light's shadow on a ground under them, the
    /// ground itself unseen ([`ground`]): the window's Shadow switch, as
    /// Moonglow Toolset's model viewer has it. Off for pictures, as the
    /// toolset's thumbnails and galleries draw none.
    pub shadow: bool,
    /// The box around what is shown at rest, where the ground lies: taken
    /// at the first step after what is shown changed.
    rest_box: Option<(Vec3, Vec3)>,
    /// The ground's model, in the background colour it was made for.
    ground: Option<([f32; 3], Arc<GpuModel>)>,
}

/// How dark the models' shadow is on the ground (an area's Shadow
/// Opacity, of 1), as the toolset's model viewer has it: an area's 0.35
/// reads faintly on the dark background.
pub const GROUND_SHADOW_OPACITY: f32 = 0.6;

/// The ground the models cast their shadow on ([`Stage::shadow`]): a 2 m
/// square about the origin at height 0, facing up alone (seen from below,
/// the model shows whole), unlit and of the background's colour, so that
/// it is unseen but for the shadow on it. The toolset's model viewer's.
fn ground_model(background: [f32; 3]) -> Model {
    use mg_mdl::{Face, Mesh, Node, NodeKind};
    let mesh = Mesh {
        diffuse: background,
        ambient: background,
        render: true,
        vertices: vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        uvs: [vec![[0.0, 0.0]; 4], vec![], vec![], vec![]],
        faces: vec![
            Face { vertices: [0, 1, 2], material: 0 },
            Face { vertices: [0, 2, 3], material: 0 },
        ],
        source: vec![0, 1, 2, 3],
        shadow: false,
        ..Default::default()
    };
    let mut root = mg_mdl::Node::new("mgv_ground", NodeKind::Dummy);
    root.children = vec![1];
    let mut m = Node::new("ground", NodeKind::Mesh(Box::new(mesh)));
    m.parent = Some(0);
    Model {
        name: "mgv_ground".into(),
        animation_scale: 1.0,
        nodes: vec![root, m],
        ..Default::default()
    }
}

/// The ground under what is shown, whose box at rest is `(min, max)`: at
/// the box's foot, or at the origin's height where the box is above it (a
/// model hung or floating casts down to where it stands), a centimetre
/// below so that a flat base on it does not meet it, wide enough for a
/// shadow about as long as the model is tall.
fn ground(model: Arc<GpuModel>, (min, max): (Vec3, Vec3)) -> Instance {
    let size = max - min;
    let half = size.x.max(size.y).max(size.z) * 1.5 + 0.5;
    let middle = (min + max) * 0.5;
    let at = Vec3::new(middle.x, middle.y, min.z.min(0.0) - 0.01);
    Instance {
        unlit: true,
        receives_shadows: true,
        casts_shadows: false,
        ..Instance::new(model, Mat4::from_translation(at) * Mat4::from_scale(Vec3::splat(half)))
    }
}

impl Stage {
    pub fn new(gpu: Gpu) -> Stage {
        Stage {
            gpu,
            actors: Vec::new(),
            chunk_models: HashMap::new(),
            chunks: Vec::new(),
            lighting: Lighting::studio(),
            model_lights: true,
            elapsed: 0.0,
            walkmeshes: Vec::new(),
            front: subject::FACING_Y,
            casters: false,
            shadow: false,
            rest_box: None,
            ground: None,
        }
    }

    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// Removes every actor.
    pub fn clear(&mut self) {
        self.actors.clear();
        self.chunks.clear();
        self.chunk_models.clear();
        self.walkmeshes.clear();
        self.elapsed = 0.0;
        self.front = subject::FACING_Y;
        self.rest_box = None;
    }

    pub fn actors(&self) -> &[Actor] {
        &self.actors
    }

    pub fn actor(&self, id: ActorId) -> Option<&Actor> {
        self.actors.get(id.0)
    }

    pub fn actor_mut(&mut self, id: ActorId) -> Option<&mut Actor> {
        self.actors.get_mut(id.0)
    }

    pub fn is_empty(&self) -> bool {
        self.actors.is_empty()
    }

    /// An actor for a model, ready to add (its animations found through the
    /// library's supermodels).
    pub fn actor_for(
        &self,
        lib: &Library,
        name: &str,
        model: Arc<Model>,
        placement: Placement,
    ) -> Actor {
        let animations = Animations(anim::animations(&model, &|n| lib.model(n)));
        let gpu_model = Arc::new(GpuModel::new(&self.gpu, model));
        Actor::new(name, gpu_model, animations, placement)
    }

    /// Adds an actor. One that hangs from another must come after it.
    pub fn add(&mut self, actor: Actor) -> ActorId {
        if let Placement::On { parent, .. } = actor.placement {
            assert!(parent.0 < self.actors.len(), "a parent comes before its children");
        }
        self.actors.push(actor);
        self.rest_box = None;
        ActorId(self.actors.len() - 1)
    }

    /// Adds a model by resource name, in the world.
    pub fn add_model(
        &mut self,
        lib: &Library,
        name: &str,
        transform: Mat4,
    ) -> Result<ActorId, StageError> {
        let model = lib.model(name).ok_or_else(|| StageError::NoModel(name.to_string()))?;
        let actor = self.actor_for(lib, name, model, Placement::World(transform));
        Ok(self.add(actor))
    }

    /// Adds a blueprint's or a 2DA row's preview: its base in the world,
    /// its parts on the base's nodes, playing its idle animation.
    pub fn add_preview(
        &mut self,
        lib: &Library,
        preview: &Preview,
        transform: Mat4,
    ) -> Result<Added, StageError> {
        let base_part = &preview.base;
        let model = lib
            .model(&base_part.model)
            .ok_or_else(|| StageError::NoModel(base_part.model.clone()))?;
        let mut base =
            self.actor_for(lib, &base_part.model, model.clone(), Placement::World(transform));
        base.look = look(base_part);
        if let Some(idle) = &preview.idle
            && base.animations.find(idle).is_some()
        {
            base.player.play(Some(idle), PlayMode::Loop);
        }
        base.carried = preview.lights.iter().map(|l| (l.offset, l.color, l.radius)).collect();
        let base = self.add(base);
        let mut parts = Vec::new();
        let mut missing = Vec::new();
        for p in &preview.parts {
            let Some(m) = lib.model(&p.model) else {
                missing.push(p.model.clone());
                continue;
            };
            let node = p.attach.as_deref().and_then(|a| model.node(a));
            let placement = Placement::On { parent: base, node, scale: p.scale, follow: true };
            let part_look = look(p);
            let mut actor = if p.animated {
                self.actor_for(lib, &p.model, m, placement)
            } else {
                // Parts that only follow a node play nothing of their own.
                let gpu_model = Arc::new(GpuModel::new(&self.gpu, m));
                Actor::new(&p.model, gpu_model, Animations::default(), placement)
            };
            actor.look = part_look;
            actor.follows_parent = p.animated;
            parts.push(self.add(actor));
        }
        Ok(Added { base, parts, missing })
    }

    /// Advances everything by `dt` seconds.
    pub fn step(&mut self, lib: &Library, dt: f32) {
        let dt = dt.max(0.0);
        self.elapsed += dt;
        let main_lights = self.lighting.main_lights;
        let main_light = |slot: usize| main_lights.get(slot).copied().flatten();
        if self.casters {
            for a in self.actors.iter_mut().filter(|a| a.casters.is_none()) {
                let view = Arc::new(actor::casters_view(&a.model.model));
                a.casters = Some(Arc::new(GpuModel::new(&self.gpu, view)));
            }
        }
        let mut chunk_requests = Vec::new();
        for i in 0..self.actors.len() {
            let (before, rest) = self.actors.split_at_mut(i);
            let actor = &mut rest[0];
            let parent = match actor.placement {
                Placement::On { parent, node, scale, follow } => {
                    before.get(parent.0).map(|p| (p, node, scale, follow))
                }
                Placement::World(_) => None,
            };

            // What it plays: its own animation, or its parent's.
            let (current, step_dt) = match parent.filter(|_| actor.follows_parent) {
                Some((p, _, _, _)) => {
                    let own = p.current.as_ref().and_then(|c| {
                        actor.animations.owner(&c.name).map(|owner| Current {
                            name: c.name.clone(),
                            owner: owner.clone(),
                            time: c.time,
                        })
                    });
                    let idle = || {
                        ["creadyl", "cpause1", "pause1"].iter().find_map(|n| {
                            let owner = actor.animations.owner(n)?;
                            Some(Current { name: n.to_string(), owner: owner.clone(), time: 0.0 })
                        })
                    };
                    let current = own.or_else(idle).or_else(|| p.current.clone());
                    let speed = if p.player.playing { p.player.speed } else { 0.0 };
                    (current, dt * speed)
                }
                None => {
                    let current = actor.advance(dt).and_then(|(name, time)| {
                        let owner = actor.animations.owner(&name)?.clone();
                        Some(Current { name, owner, time })
                    });
                    let speed = if actor.player.playing { actor.player.speed } else { 0.0 };
                    (current, dt * speed)
                }
            };
            let anim = current.as_ref().and_then(Current::animation);
            let t = current.as_ref().map_or(0.0, |c| c.time);
            // (A part that follows its wearer plays the wearer's
            // animation at the wearer's scale.)
            let worn = parent
                .filter(|_| actor.follows_parent)
                .map(|(p, ..)| p.model.model.animation_scale);
            let (locals, mut state) = match worn {
                Some(worn) => actor.sample_worn(anim, t, worn),
                None => actor.sample(anim, t),
            };
            let name = |c: &Option<Current>| c.as_ref().map(|c| c.name.to_ascii_lowercase());
            let changed = (name(&current) != name(&actor.current)).then_some(anim);
            let pose = actor.transition(changed, locals, step_dt);

            let world = match (actor.placement, parent) {
                (Placement::World(m), _) => m,
                (Placement::On { .. }, Some((p, node, scale, follow))) => {
                    let at = node.and_then(|n| p.pose.get(n).copied()).unwrap_or(Mat4::IDENTITY);
                    let hook = p.world * at;
                    let hook = if follow {
                        hook
                    } else {
                        // Where the node is, turned as the parent is.
                        let (_, turn, _) = p.world.to_scale_rotation_translation();
                        Mat4::from_rotation_translation(turn, hook.w_axis.truncate())
                    };
                    hook * Mat4::from_scale(Vec3::splat(scale))
                }
                (Placement::On { .. }, None) => Mat4::IDENTITY,
            };

            actor.dangly.update(step_dt, &pose, world, Vec3::ZERO);
            actor.dangly.apply(&mut state, &pose, world);
            let model = actor.model.model.clone();
            actor.particles.update(&model, anim, t, step_dt, &pose, world);
            for c in actor.particles.chunks(&model, anim, t, &pose, world) {
                chunk_requests.push(c);
            }
            let mut lights = anim::lights(&model, anim, t, &pose, world, &main_light);
            for &(offset, color, radius) in &actor.carried {
                lights.push(PointLight::new(
                    world.transform_point3(offset),
                    color,
                    radius,
                    false,
                    4,
                ));
            }

            actor.world = world;
            actor.pose = Arc::new(pose);
            actor.state = Arc::new(state);
            actor.lights = lights;
            actor.current = current;
        }

        self.chunks.clear();
        for c in chunk_requests {
            let gpu = &self.gpu;
            let gm = self
                .chunk_models
                .entry(c.model.clone())
                .or_insert_with(|| lib.model(&c.model).map(|m| Arc::new(GpuModel::new(gpu, m))))
                .clone();
            if let Some(gm) = gm {
                self.chunks.push(Instance::new(gm, c.transform));
            }
        }
        // (The ground under it, with the Shadow switch on: where it stood
        // at its first step, in the background's colour.)
        if self.shadow {
            if self.rest_box.is_none() {
                self.rest_box = self.bounds();
            }
            let background = self.lighting.background;
            if self.ground.as_ref().is_none_or(|(b, _)| *b != background) {
                let model = Arc::new(ground_model(background));
                self.ground = Some((background, Arc::new(GpuModel::new(&self.gpu, model))));
            }
        }
    }

    /// The renderer's scene, for a camera with this view matrix (particles
    /// face it).
    pub fn scene(&self, view: Mat4) -> Scene {
        let mut instances = Vec::new();
        let mut particles = Vec::new();
        let mut lights = self.lighting.lights.clone();
        for a in self.actors.iter().filter(|a| a.visible) {
            let anim = a.current.as_ref().and_then(Current::animation);
            let t = a.current.as_ref().map_or(0.0, |c| c.time);
            if self.casters {
                // The casters alone, posed as the model is.
                if let Some(c) = &a.casters {
                    instances.push(Instance {
                        pose: Some(a.pose.clone()),
                        ..Instance::new(c.clone(), a.world)
                    });
                }
                continue;
            }
            instances.push(Instance {
                pose: Some(a.pose.clone()),
                state: Some(a.state.clone()),
                env_map: a.look.env_map.clone(),
                plt_colors: a.look.colors,
                textures: a.look.textures.clone(),
                ..Instance::new(a.model.clone(), a.world)
            });
            particles.extend(a.particles.batches(&a.model.model, anim, t, &a.pose, a.world, view));
            if self.model_lights {
                lights.extend(a.lights.iter().cloned());
            }
        }
        if !self.casters {
            instances.extend(self.chunks.iter().cloned());
        }
        let ground = match (&self.ground, self.rest_box) {
            (Some((_, model)), Some(rest)) if self.shadow && !self.casters => {
                Some(ground(model.clone(), rest))
            }
            _ => None,
        };
        instances.extend(ground.clone());
        Scene {
            instances,
            lights,
            area: self.lighting.area,
            fog: self.lighting.fog,
            background: self.lighting.background,
            particles,
            env_map: self.lighting.env_map.clone(),
            sky: None,
            sky_fade: None,
            lines: Vec::new(),
            time: self.elapsed,
            // (No area: the game's shaders read no wind, weather or
            // module clock of one; noon of a new module's day.)
            environment: Default::default(),
            // (The area light's on the ground, with the Shadow switch on,
            // laid as the toolset's model viewer lays them: as far up the
            // picture as the fog lets them, else all the way.)
            shadows: ground.is_some().then(|| mg_render::SunShadows {
                opacity: GROUND_SHADOW_OPACITY,
                fade_limit: self.lighting.fog.map_or(10_000.0, |f| f.end),
            }),
            light_shadows: None,
            line_shapes: Vec::new(),
        }
    }

    /// Bounds of everything visible, as of the last step: of the meshes'
    /// vertices where they are drawn (a skin's follow its bones, which its
    /// node's own box does not: a dragon's reached twice its height).
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for p in self.posed(false).iter().flat_map(|m| &m.positions).filter(|p| p.is_finite()) {
            min = min.min(*p);
            max = max.max(*p);
        }
        (min.x <= max.x).then_some((min, max))
    }

    /// Where everything showing is (as of the last step): the drawn meshes'
    /// vertices and the particles, so emitter-only effects count too; and
    /// where nothing is drawn, the meshes the renderer skips and the
    /// walkmeshes (a walkmesh opened alone).
    pub fn shown_points(&self) -> Vec<Vec3> {
        let scene = self.scene(Mat4::IDENTITY);
        let mut out: Vec<Vec3> = self.posed(false).into_iter().flat_map(|m| m.positions).collect();
        if out.is_empty() {
            out = self
                .posed(true)
                .into_iter()
                .chain(self.posed_walkmeshes())
                .flat_map(|m| m.positions)
                .collect();
        }
        out.extend(scene.particles.iter().flat_map(|b| &b.vertices).map(|v| Vec3::from(v.pos)));
        out.retain(|p| p.is_finite());
        out
    }

    /// Bounds of [`Stage::shown_points`].
    pub fn bounds_with_particles(&self) -> Option<(Vec3, Vec3)> {
        self.shown_points().into_iter().fold(None, |b, p| match b {
            Some((min, max)) => Some((min.min(p), max.max(p))),
            None => Some((p, p)),
        })
    }

    /// Whether anything moves without input: animations playing, live
    /// particles, swinging dangly meshes.
    pub fn animating(&self) -> bool {
        self.actors.iter().any(|a| {
            (a.player.playing && a.current.is_some()) || a.particles_live() || a.dangly_moving()
        })
    }

    /// Whether any actor plays (not paused): what moves by the clock moves.
    pub fn playing(&self) -> bool {
        self.actors.iter().any(|a| a.player.playing)
    }

    /// Picks up changed models from the library (after an edit, a file
    /// changed on disk or a layer added): actors whose model changed get
    /// the new one, keeping what they play; every actor's animations are
    /// looked up again (a supermodel may have changed). Returns how many
    /// actors changed model.
    pub fn reload(&mut self, lib: &Library) -> usize {
        let mut changed = 0;
        for i in 0..self.actors.len() {
            let name = self.actors[i].name.clone();
            let Some(model) = lib.model(&name) else { continue };
            let follows = !self.actors[i].animations.0.is_empty()
                || matches!(self.actors[i].placement, Placement::World(_));
            let animations = if follows {
                Animations(anim::animations(&model, &|n| lib.model(n)))
            } else {
                Animations::default()
            };
            if *model != *self.actors[i].model.model {
                let gm = Arc::new(GpuModel::new(&self.gpu, model));
                self.actors[i].replace_model(gm, animations);
                changed += 1;
            } else {
                self.actors[i].animations = animations;
            }
        }
        self.chunk_models.clear();
        if changed > 0 {
            self.rest_box = None;
        }
        changed
    }

    /// Replaces an actor's model with one not found by name (a file whose
    /// name is not a resource name), keeping what it plays.
    pub fn replace_model(&mut self, lib: &Library, id: ActorId, model: Arc<Model>) {
        let animations = Animations(anim::animations(&model, &|n| lib.model(n)));
        let gm = Arc::new(GpuModel::new(&self.gpu, model));
        if let Some(a) = self.actors.get_mut(id.0) {
            a.replace_model(gm, animations);
        }
    }

    /// Steps from the start to `time` in fixed steps of `1 / fps` seconds
    /// (for headless renders: the same request gives the same frame).
    pub fn settle(&mut self, lib: &Library, time: f32, fps: f32) {
        let dt = 1.0 / fps.max(1.0);
        // A zero step first, so poses exist at time 0.
        self.step(lib, 0.0);
        let steps = (time / dt).round() as usize;
        for _ in 0..steps {
            self.step(lib, dt);
        }
    }

    /// Every actor's mesh states, for callers that pose meshes themselves
    /// (overlays, picking).
    pub fn mesh_states(&self) -> impl Iterator<Item = (&Actor, &MeshState)> {
        self.actors.iter().map(|a| (a, a.state.as_ref()))
    }
}

/// Adds a body part's fallback textures ([`textures::part_fallbacks`]) to
/// a look, under what it already replaces.
pub(crate) fn part_textures(look: &mut Look, lib: &Library, name: &str, model: &Model) {
    let fallbacks = textures::part_fallbacks(lib.resman(), model, name);
    if fallbacks.is_empty() {
        return;
    }
    let mut map = look.textures.as_deref().cloned().unwrap_or_default();
    for (from, to) in fallbacks {
        map.entry(from).or_insert(to);
    }
    look.textures = Some(Arc::new(map));
}

fn look(p: &mg_preview::Part) -> Look {
    Look {
        textures: (!p.textures.is_empty())
            .then(|| Arc::new(p.textures.iter().map(|(a, b)| (a.clone(), b.clone())).collect())),
        colors: p.colors,
        env_map: p.env_map.clone(),
    }
}
