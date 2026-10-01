//! An actor: one model instance with its animation, particles, dangly
//! meshes and lights, placed in the world or hanging from another actor's
//! node.

use std::collections::VecDeque;
use std::sync::Arc;

use glam::{Mat4, Vec3};
use mg_mdl::{Animation, Model};
use mg_render::anim::Local;
use mg_render::dangly::Dangly;
use mg_render::particles::Particles;
use mg_render::{GpuModel, MeshState, PointLight, anim};

/// An actor's index in its stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActorId(pub usize);

/// How an animation plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayMode {
    /// Round and round.
    #[default]
    Loop,
    /// Once, then the next queued animation, or hold the last frame.
    Once,
}

/// What an actor is playing and where it is in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    /// `None`: the rest pose.
    pub animation: Option<String>,
    pub mode: PlayMode,
    /// Seconds into the animation.
    pub time: f32,
    /// Playback speed (1 = as authored).
    pub speed: f32,
    pub playing: bool,
    /// What plays after a `Once` animation ends.
    pub queue: VecDeque<(String, PlayMode)>,
}

impl Default for Player {
    fn default() -> Player {
        Player {
            animation: None,
            mode: PlayMode::Loop,
            time: 0.0,
            speed: 1.0,
            playing: true,
            queue: VecDeque::new(),
        }
    }
}

impl Player {
    /// Starts an animation from its beginning, clearing the queue.
    pub fn play(&mut self, animation: Option<&str>, mode: PlayMode) {
        self.animation = animation.map(str::to_string);
        self.mode = mode;
        self.time = 0.0;
        self.queue.clear();
    }

    /// Plays animations one after another (each but the last once).
    pub fn sequence(&mut self, names: &[&str]) {
        let Some((first, rest)) = names.split_first() else {
            self.play(None, PlayMode::Loop);
            return;
        };
        let mode = |last: bool| if last { PlayMode::Loop } else { PlayMode::Once };
        self.play(Some(first), mode(rest.is_empty()));
        for (i, n) in rest.iter().enumerate() {
            self.queue.push_back((n.to_string(), mode(i + 1 == rest.len())));
        }
    }
}

/// The animations an actor can play: its model's own and its supermodels'.
#[derive(Debug, Clone, Default)]
pub struct Animations(pub Vec<(String, Arc<Model>)>);

impl Animations {
    /// The model that owns an animation (the nearest in the chain).
    pub fn owner(&self, name: &str) -> Option<&Arc<Model>> {
        self.0.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, m)| m)
    }

    pub fn find(&self, name: &str) -> Option<&Animation> {
        self.0
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .and_then(|(_, owner)| owner.animation(name))
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(n, _)| n.as_str())
    }
}

/// How an actor looks beyond its model: a blueprint part's replaced
/// textures, PLT colours and environment map.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Look {
    pub textures: Option<Arc<std::collections::HashMap<String, String>>>,
    pub colors: Option<[u8; 10]>,
    pub env_map: Option<String>,
}

/// Where an actor is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// In the world, at this transform.
    World(Mat4),
    /// On another actor: at one of its nodes (`None`: its origin), scaled.
    /// `follow`: turn with the node; otherwise only its position counts,
    /// turned as the parent actor is (visual effects that do not orient
    /// with their object).
    On { parent: ActorId, node: Option<usize>, scale: f32, follow: bool },
}

/// A model instance on the stage.
#[derive(Debug)]
pub struct Actor {
    /// The model's resource name (lower case), or what to call it.
    pub name: String,
    pub model: Arc<GpuModel>,
    pub animations: Animations,
    pub player: Player,
    pub placement: Placement,
    pub look: Look,
    /// Plays its parent's animation on its own nodes when it has none of
    /// its own by that name (skinned robes and cloaks, wings, tails).
    pub follows_parent: bool,
    pub visible: bool,
    pub(crate) particles: Particles,
    pub(crate) dangly: Dangly,
    // What the last step computed.
    pub(crate) world: Mat4,
    /// The nodes' transforms relative to their parents, for transitions.
    pub(crate) locals: Vec<Local>,
    /// A transition into the current animation.
    pub(crate) transition: Option<Transition>,
    pub(crate) pose: Arc<Vec<Mat4>>,
    pub(crate) state: Arc<MeshState>,
    pub(crate) lights: Vec<PointLight>,
    /// The animation the last step used.
    pub(crate) current: Option<Current>,
    /// Lights the actor carries besides its model's (a placeable
    /// blueprint's light): offset from its origin, colour, radius.
    pub carried: Vec<(Vec3, Vec3, f32)>,
}

/// A transition from a pose into an animation over its `transtime`.
#[derive(Debug, Clone)]
pub(crate) struct Transition {
    pub(crate) from: Vec<Local>,
    pub(crate) elapsed: f32,
    pub(crate) length: f32,
}

/// The animation an actor's last step used: its name, the model that owns
/// it and the time in it.
#[derive(Debug, Clone)]
pub(crate) struct Current {
    pub(crate) name: String,
    pub(crate) owner: Arc<Model>,
    pub(crate) time: f32,
}

impl Current {
    pub(crate) fn animation(&self) -> Option<&Animation> {
        self.owner.animation(&self.name)
    }
}

impl Actor {
    pub fn new(
        name: &str,
        model: Arc<GpuModel>,
        animations: Animations,
        placement: Placement,
    ) -> Actor {
        let particles = Particles::new(&model.model);
        let dangly = Dangly::new(&model);
        let pose = Arc::new(model.rest.clone());
        let state = Arc::new(MeshState::new(&model));
        Actor {
            name: name.to_ascii_lowercase(),
            model,
            animations,
            player: Player::default(),
            placement,
            look: Look::default(),
            follows_parent: false,
            visible: true,
            particles,
            dangly,
            world: Mat4::IDENTITY,
            locals: Vec::new(),
            transition: None,
            pose,
            state,
            lights: Vec::new(),
            current: None,
            carried: Vec::new(),
        }
    }

    /// Swaps in a new version of the model (an edit, a reload), keeping what
    /// it plays and where it is.
    pub fn replace_model(&mut self, model: Arc<GpuModel>, animations: Animations) {
        self.particles = Particles::new(&model.model);
        self.dangly = Dangly::new(&model);
        self.pose = Arc::new(model.rest.clone());
        self.state = Arc::new(MeshState::new(&model));
        self.locals.clear();
        self.transition = None;
        self.model = model;
        self.animations = animations;
    }

    /// Restarts its particles (emitters fire again from nothing).
    pub fn restart_particles(&mut self) {
        self.particles = Particles::new(&self.model.model);
    }

    /// World transform from the last step.
    pub fn world(&self) -> Mat4 {
        self.world
    }

    /// Model-space node transforms from the last step.
    pub fn pose(&self) -> &[Mat4] {
        &self.pose
    }

    /// The animation and time the last step used.
    pub fn current(&self) -> Option<(&str, f32)> {
        self.current.as_ref().map(|c| (c.name.as_str(), c.time))
    }

    /// The lights the last step produced (world space).
    pub fn lights(&self) -> &[PointLight] {
        &self.lights
    }

    /// Whether particles are showing.
    pub fn particles_live(&self) -> bool {
        self.particles.live()
    }

    /// Whether a dangly mesh is still swinging.
    pub fn dangly_moving(&self) -> bool {
        self.dangly.moving()
    }

    /// Advances the player by `dt` seconds of real time; returns the
    /// animation to sample and the time in it.
    pub(crate) fn advance(&mut self, dt: f32) -> Option<(String, f32)> {
        let p = &mut self.player;
        if p.playing {
            p.time += dt * p.speed;
        }
        loop {
            let name = p.animation.clone()?;
            let a = self.animations.find(&name)?;
            if p.mode == PlayMode::Once && p.time >= a.length {
                if let Some((next, mode)) = p.queue.pop_front() {
                    p.time -= a.length;
                    p.animation = Some(next);
                    p.mode = mode;
                    continue;
                }
                // Hold the last frame (times within the animation, its
                // length included, are taken as they are).
                return Some((name, a.length));
            }
            return Some((name, p.time));
        }
    }

    /// The nodes' transforms (relative to their parents) and the mesh state
    /// for an animation at a time.
    pub(crate) fn sample(&self, anim: Option<&Animation>, t: f32) -> (Vec<Local>, MeshState) {
        match anim {
            Some(a) => (anim::locals(&self.model.model, a, t), anim::mesh_state(&self.model, a, t)),
            None => (anim::rest_locals(&self.model.model), MeshState::new(&self.model)),
        }
    }

    /// The pose for this step: `locals`, or on the way to them from the
    /// last pose when the animation changed (over the new one's
    /// `transtime`, as the game blends).
    pub(crate) fn transition(
        &mut self,
        changed_to: Option<Option<&Animation>>,
        mut locals: Vec<Local>,
        dt: f32,
    ) -> Vec<Mat4> {
        if let Some(anim) = changed_to {
            let length = anim.map_or(0.0, |a| a.transtime);
            self.transition = (length > 0.0 && self.locals.len() == locals.len())
                .then(|| Transition { from: self.locals.clone(), elapsed: 0.0, length });
        }
        if let Some(t) = &mut self.transition {
            t.elapsed += dt;
            if t.elapsed >= t.length {
                self.transition = None;
            } else {
                locals = anim::blend(&t.from, &locals, t.elapsed / t.length);
            }
        }
        let pose = anim::compose(&self.model.model, &locals);
        self.locals = locals;
        pose
    }

    /// Model bounds in world space for the last step's pose (meshes' boxes
    /// through their nodes).
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for m in &self.model.meshes {
            let node = self.pose.get(m.node).copied().unwrap_or(Mat4::IDENTITY);
            let to = self.world * node;
            for c in 0..8 {
                let p = Vec3::new(
                    if c & 1 == 0 { m.min.x } else { m.max.x },
                    if c & 2 == 0 { m.min.y } else { m.max.y },
                    if c & 4 == 0 { m.min.z } else { m.max.z },
                );
                let w = to.transform_point3(p);
                min = min.min(w);
                max = max.max(w);
            }
        }
        (min.x <= max.x).then_some((min, max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequences_queue_all_but_the_last_once() {
        let mut p = Player::default();
        p.sequence(&["impact", "duration"]);
        assert_eq!(p.animation.as_deref(), Some("impact"));
        assert_eq!(p.mode, PlayMode::Once);
        assert_eq!(p.queue, VecDeque::from([("duration".to_string(), PlayMode::Loop)]));
        p.sequence(&[]);
        assert_eq!(p.animation, None);
    }
}
