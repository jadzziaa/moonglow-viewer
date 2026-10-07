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
pub use render::Viewport;

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
            let (locals, mut state) = actor.sample(anim, t);
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
        instances.extend(self.chunks.iter().cloned());
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
        }
    }

    /// Bounds of everything visible, as of the last step.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        self.actors
            .iter()
            .filter(|a| a.visible)
            .filter_map(Actor::bounds)
            .reduce(|(amin, amax), (bmin, bmax)| (amin.min(bmin), amax.max(bmax)))
    }

    /// Bounds of meshes and of the particles showing (as of the last step),
    /// so emitter-only effects frame too.
    pub fn bounds_with_particles(&self) -> Option<(Vec3, Vec3)> {
        let scene = self.scene(Mat4::IDENTITY);
        let mut out = self.bounds();
        // Walkmeshes, and meshes the renderer skips, when nothing else is
        // there (a walkmesh opened alone).
        let hidden: Vec<Vec3> = if out.is_none() {
            self.posed(true)
                .into_iter()
                .chain(self.posed_walkmeshes())
                .flat_map(|m| m.positions)
                .collect()
        } else {
            Vec::new()
        };
        let particles = scene.particles.iter().flat_map(|b| &b.vertices).map(|v| Vec3::from(v.pos));
        for p in particles.chain(hidden) {
            if !p.is_finite() {
                continue;
            }
            out = Some(match out {
                Some((min, max)) => (min.min(p), max.max(p)),
                None => (p, p),
            });
        }
        out
    }

    /// Whether anything moves without input: animations playing, live
    /// particles, swinging dangly meshes.
    pub fn animating(&self) -> bool {
        self.actors.iter().any(|a| {
            (a.player.playing && a.current.is_some()) || a.particles_live() || a.dangly_moving()
        })
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
