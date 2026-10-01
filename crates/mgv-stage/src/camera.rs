//! The viewer's camera: orbiting a target, as the toolset's model viewer
//! turns it (left drag orbits, right or middle drag pans, the wheel zooms).
//! Z is up. The views are taken from the shown model's front
//! ([`OrbitCamera::front`]): creatures face +Y, most placeables −Y
//! ([`crate::subject::front`]).

use glam::Vec3;
use mg_render::Camera;

/// Views to jump to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
    /// From the front, a little to the side and above (the default).
    ThreeQuarter,
}

/// A camera orbiting `target`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub distance: f32,
    /// Around Z from +X, radians.
    pub yaw: f32,
    /// Up from the ground plane, radians.
    pub pitch: f32,
    /// Vertical field of view, radians.
    pub fov_y: f32,
    /// Which way the shown model faces (yaw, radians around Z from +X):
    /// the views are taken from it.
    pub front: f32,
}

impl Default for OrbitCamera {
    fn default() -> OrbitCamera {
        let mut c = OrbitCamera {
            target: Vec3::ZERO,
            distance: 5.0,
            yaw: 0.0,
            pitch: 0.0,
            fov_y: 40f32.to_radians(),
            front: crate::subject::FACING_Y,
        };
        c.set_view(View::ThreeQuarter);
        c
    }
}

/// Pitch stops short of straight up and down, where the view's up vector
/// (Z) would be undefined.
const PITCH_LIMIT: f32 = 1.55;

impl OrbitCamera {
    /// The renderer's camera.
    pub fn camera(&self) -> Camera {
        let (yaw, pitch) = (self.yaw, self.pitch);
        let dir = Vec3::new(pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin());
        Camera {
            eye: self.target + dir * self.distance,
            target: self.target,
            fov_y: self.fov_y,
            near: (self.distance * 0.01).max(0.01),
            far: self.distance * 20.0 + 100.0,
        }
    }

    /// Looks from a side of the model (relative to its front).
    pub fn set_view(&mut self, view: View) {
        let d = f32::to_radians;
        let (turn, pitch) = match view {
            View::Front => (0.0, 0.0),
            View::Back => (d(180.0), 0.0),
            View::Left => (d(90.0), 0.0),
            View::Right => (d(-90.0), 0.0),
            View::Top => (0.0, PITCH_LIMIT),
            View::Bottom => (0.0, -PITCH_LIMIT),
            View::ThreeQuarter => (d(-30.0), d(20.0)),
        };
        (self.yaw, self.pitch) = (self.front + turn, pitch);
    }

    /// A model facing elsewhere: the camera turns with it, keeping its
    /// angle to the front.
    pub fn set_front(&mut self, front: f32) {
        self.yaw += front - self.front;
        self.front = front;
    }

    /// Fits a box in the view (its bounding sphere, with a margin).
    pub fn frame(&mut self, min: Vec3, max: Vec3) {
        self.target = (min + max) * 0.5;
        let radius = ((max - min).length() * 0.5).max(0.05);
        self.distance = radius / (self.fov_y * 0.5).sin() * 1.1;
    }

    /// Orbits by a drag of `dx`, `dy` points.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.01;
        self.pitch = (self.pitch + dy * 0.01).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Pans by a drag of `dx`, `dy` points.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let cam = self.camera();
        let fwd = (cam.target - cam.eye).normalize_or_zero();
        let right = fwd.cross(Vec3::Z).normalize_or_zero();
        let up = right.cross(fwd);
        let scale = self.distance * 0.0015;
        self.target += (-right * dx + up * dy) * scale;
    }

    /// Zooms by a wheel movement (positive: closer).
    pub fn zoom(&mut self, scroll: f32) {
        self.distance = (self.distance * (-scroll * 0.002).exp()).clamp(0.02, 5000.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_looks_at_the_models_face() {
        let mut c = OrbitCamera::default();
        c.set_view(View::Front);
        // A creature faces +Y: the front view's eye is on +Y.
        let cam = c.camera();
        assert!(cam.eye.y > 4.9 && cam.eye.x.abs() < 1e-4, "{}", cam.eye);
        // A placeable facing −Y: the camera turns with it.
        c.set_front(-std::f32::consts::FRAC_PI_2);
        let cam = c.camera();
        assert!(cam.eye.y < -4.9 && cam.eye.x.abs() < 1e-4, "{}", cam.eye);
        // Its left is +X (looking along −Y), and the three-quarter view
        // stays on its front.
        c.set_view(View::Left);
        assert!(c.camera().eye.x > 4.9, "{}", c.camera().eye);
        c.set_view(View::ThreeQuarter);
        assert!(c.camera().eye.y < -3.0, "{}", c.camera().eye);
    }

    #[test]
    fn framing_fits_the_box() {
        let mut c = OrbitCamera::default();
        c.frame(Vec3::splat(-1.0), Vec3::splat(1.0));
        assert_eq!(c.target, Vec3::ZERO);
        let radius = 3f32.sqrt();
        assert!((c.distance - radius / 20f32.to_radians().sin() * 1.1).abs() < 1e-4);
    }

    #[test]
    fn pitch_stays_short_of_the_poles() {
        let mut c = OrbitCamera::default();
        c.orbit(0.0, 1e6);
        assert!(c.pitch <= PITCH_LIMIT);
        c.zoom(1e9);
        assert!(c.distance >= 0.02);
    }
}
