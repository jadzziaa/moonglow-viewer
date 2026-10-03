//! Rendering without a window: stills and turntables, stepped at fixed
//! intervals so the same request gives the same image.

use glam::Vec3;
use mg_image::Rgba;
use mg_render::PointLight;
use mgv_library::Library;

use crate::camera::{OrbitCamera, View};
use crate::{Stage, Viewport};

/// How to take a picture of the stage.
#[derive(Debug, Clone, PartialEq)]
pub struct Shot {
    /// Width and height in pixels.
    pub size: (u32, u32),
    /// Seconds of animation (and particles) before the picture.
    pub time: f32,
    /// Steps per second while getting there.
    pub fps: f32,
    pub view: View,
    /// Overrides the view's angles (degrees; the yaw around Z from +X,
    /// whichever way the model faces).
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    /// Distance as a multiple of the framed distance (1: the bounds fill
    /// the picture).
    pub zoom: f32,
    pub background: Option<[f32; 3]>,
    /// A light at the camera, so the sides in view are lit whatever the
    /// sun's direction (a mesh with no ambient colour is black out of the
    /// sun): its linear strength, 0 (none) to 1. About 0.3 is as strong as
    /// the studio sun. It reaches `KEY_LIGHT_REACH` times the camera's
    /// distance, so it falls off little across the model.
    pub key_light: f32,
}

/// How far the key light reaches, as a multiple of the camera's distance.
pub const KEY_LIGHT_REACH: f32 = 10.0;

impl Default for Shot {
    fn default() -> Shot {
        Shot {
            size: (512, 512),
            time: 0.0,
            fps: 30.0,
            view: View::ThreeQuarter,
            yaw: None,
            pitch: None,
            zoom: 1.0,
            background: None,
            key_light: 0.0,
        }
    }
}

impl Shot {
    /// The camera for the stage's current bounds.
    pub fn camera(&self, stage: &Stage) -> OrbitCamera {
        let mut cam = OrbitCamera { front: stage.front, ..OrbitCamera::default() };
        cam.set_view(self.view);
        if let Some(y) = self.yaw {
            cam.yaw = y.to_radians();
        }
        if let Some(p) = self.pitch {
            cam.pitch = p.to_radians().clamp(-1.55, 1.55);
        }
        let (min, max) =
            stage.bounds_with_particles().unwrap_or((Vec3::splat(-1.0), Vec3::splat(1.0)));
        cam.frame(min, max);
        cam.distance *= self.zoom.max(0.01);
        cam
    }

    /// The key light for a camera, when the shot has one: white, at the
    /// camera, the highest priority (kept however many lights the models
    /// bring).
    pub fn key_light(&self, cam: &OrbitCamera) -> Option<PointLight> {
        (self.key_light > 0.0).then(|| PointLight {
            position: cam.camera().eye,
            color: Vec3::splat(self.key_light.min(1.0)),
            cutoff: cam.distance * KEY_LIGHT_REACH,
            ambient_only: false,
            priority: 1,
        })
    }
}

/// Steps the stage to the shot's time (from the start) and renders it.
pub fn still(stage: &mut Stage, viewport: &mut Viewport, lib: &Library, shot: &Shot) -> Rgba {
    stage.settle(lib, shot.time, shot.fps);
    picture(stage, viewport, lib, shot, &shot.camera(stage))
}

/// Renders the stage as it is, from a camera.
pub fn picture(
    stage: &Stage,
    viewport: &mut Viewport,
    lib: &Library,
    shot: &Shot,
    cam: &OrbitCamera,
) -> Rgba {
    let camera = cam.camera();
    let mut scene = stage.scene(camera.view());
    if let Some(bg) = shot.background {
        scene.background = bg;
    }
    scene.lights.extend(shot.key_light(cam));
    viewport.image(stage.gpu(), lib.resman(), &scene, &camera, shot.size)
}

/// `frames` pictures turning once around the stage, the animation running
/// at `fps` between them (after the shot's time). The camera is framed
/// once, on the bounds at the start, so the model does not jump.
pub fn turntable(
    stage: &mut Stage,
    viewport: &mut Viewport,
    lib: &Library,
    shot: &Shot,
    frames: usize,
) -> Vec<Rgba> {
    stage.settle(lib, shot.time, shot.fps);
    let mut cam = shot.camera(stage);
    let start = cam.yaw;
    let frames = frames.max(1);
    let mut out = Vec::with_capacity(frames);
    for i in 0..frames {
        cam.yaw = start + std::f32::consts::TAU * i as f32 / frames as f32;
        out.push(picture(stage, viewport, lib, shot, &cam));
        stage.step(lib, 1.0 / shot.fps.max(1.0));
    }
    out
}

/// Frames as an animated PNG that loops forever, `fps` frames a second.
pub fn apng(frames: &[Rgba], fps: f32) -> Vec<u8> {
    let Some(first) = frames.first() else { return Vec::new() };
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, first.width, first.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_animated(frames.len() as u32, 0).expect("frame count");
        let (num, den) = frame_delay(fps);
        enc.set_frame_delay(num, den).expect("delay");
        let mut w = enc.write_header().expect("writing to memory");
        for f in frames {
            w.write_image_data(&f.data).expect("writing to memory");
        }
        w.finish().expect("writing to memory");
    }
    out
}

/// A frame time as a fraction of a second with 16-bit parts.
fn frame_delay(fps: f32) -> (u16, u16) {
    let fps = fps.clamp(1.0, 240.0);
    let den = 1000u16;
    ((1000.0 / fps).round() as u16, den)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_light_sits_at_the_camera() {
        let cam = OrbitCamera { distance: 4.0, ..OrbitCamera::default() };
        assert_eq!(Shot::default().key_light(&cam), None);
        let shot = Shot { key_light: 2.0, ..Shot::default() };
        let light = shot.key_light(&cam).unwrap();
        assert_eq!(light.position, cam.camera().eye);
        assert_eq!(light.color, Vec3::ONE, "at most 1");
        assert_eq!(light.cutoff, 4.0 * KEY_LIGHT_REACH);
        assert!(!light.ambient_only);
        assert_eq!(light.priority, 1);
    }

    #[test]
    fn delays() {
        assert_eq!(frame_delay(25.0), (40, 1000));
        assert_eq!(frame_delay(0.0), (1000, 1000));
    }
}
