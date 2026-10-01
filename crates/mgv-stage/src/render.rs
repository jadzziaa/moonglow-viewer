//! Drawing a stage: into targets a window shows, or offscreen into an
//! image.

use std::io::Write;
use std::path::Path;

use mg_image::Rgba;
use mg_render::{Assets, Camera, Gpu, Renderer, Scene, Targets};

use crate::overlay::{Overlay, OverlayPass};

/// The colour format the viewer draws in. The renderer writes gamma-space
/// values itself, so the target is not sRGB.
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Multisampling for 3D views and offscreen renders.
pub const SAMPLES: u32 = 4;

/// A renderer with its own targets, resized to what is asked.
#[derive(Debug)]
pub struct Viewport {
    renderer: Renderer,
    overlays: OverlayPass,
    targets: Option<Targets>,
}

impl Viewport {
    pub fn new(gpu: &Gpu) -> Viewport {
        Viewport {
            renderer: Renderer::new(gpu, FORMAT, SAMPLES),
            overlays: OverlayPass::new(gpu),
            targets: None,
        }
    }

    pub fn renderer(&mut self) -> &mut Renderer {
        &mut self.renderer
    }

    /// Forgets cached textures (after the library changed).
    pub fn clear_textures(&mut self) {
        self.renderer.clear_textures();
    }

    /// The targets, made or remade at `size`; whether they are new.
    pub fn targets(&mut self, gpu: &Gpu, size: (u32, u32)) -> (&Targets, bool) {
        let size = (size.0.clamp(1, 8192), size.1.clamp(1, 8192));
        let fresh = self.targets.as_ref().is_none_or(|t| t.size != size);
        if fresh {
            self.targets = Some(Targets::new(gpu, FORMAT, SAMPLES, size.0, size.1));
        }
        (self.targets.as_ref().expect("made above"), fresh)
    }

    /// The targets as last drawn.
    pub fn current(&self) -> Option<&Targets> {
        self.targets.as_ref()
    }

    /// Draws a scene into the targets (made at `size` if needed).
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        assets: &dyn Assets,
        scene: &Scene,
        camera: &Camera,
        size: (u32, u32),
    ) -> &Targets {
        self.targets(gpu, size);
        let t = self.targets.as_ref().expect("made above");
        self.renderer.render(
            gpu,
            assets,
            scene,
            camera,
            t.render_view(),
            t.resolve_view(),
            &t.depth,
            t.size,
        );
        t
    }

    /// Draws overlays over the last frame (drawn with `camera`), tested
    /// against its depths.
    pub fn draw_overlay(&self, gpu: &Gpu, overlay: &Overlay, camera: &Camera) {
        if let Some(t) = &self.targets {
            self.overlays.draw(gpu, overlay, camera, t);
        }
    }

    /// Draws a scene and reads it back (rows top first).
    pub fn image(
        &mut self,
        gpu: &Gpu,
        assets: &dyn Assets,
        scene: &Scene,
        camera: &Camera,
        size: (u32, u32),
    ) -> Rgba {
        let t = self.draw(gpu, assets, scene, camera, size);
        gpu.read_rgba(&t.color)
    }
}

/// An image (rows top first) as PNG bytes.
pub fn png_bytes(img: &Rgba) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.width, img.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().expect("writing to memory");
        w.write_image_data(&img.data).expect("writing to memory");
    }
    out
}

/// Writes an image (rows top first) as a PNG file.
pub fn save_png(img: &Rgba, path: &Path) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    f.write_all(&png_bytes(img))?;
    f.flush()
}
