//! Drawing a stage: into targets a window shows, or offscreen into an
//! image.

use std::io::Write;
use std::path::{Path, PathBuf};

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

/// Where the game's compiled programs are kept between runs
/// ([`set_shader_cache`]).
static SHADER_CACHE: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();

/// Keeps the game's programs, once compiled, in `dir` for every viewport
/// made after (the programs' own cache folder; the first call holds).
/// Without it they are compiled again in each run.
pub fn set_shader_cache(dir: Option<PathBuf>) {
    let _ = SHADER_CACHE.set(dir);
}

/// The viewer's cache folder for the game's compiled programs:
/// `%LOCALAPPDATA%\Moonglow Viewer\shaders`, `~/Library/Caches/Moonglow
/// Viewer/shaders`, or `$XDG_CACHE_HOME/moonglow-viewer/shaders`
/// (`~/.cache/moonglow-viewer/shaders`).
pub fn shader_cache_dir() -> Option<PathBuf> {
    let var = |v: &str| std::env::var_os(v).map(PathBuf::from);
    let base = if cfg!(target_os = "windows") {
        var("LOCALAPPDATA").map(|d| d.join("Moonglow Viewer"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Caches/Moonglow Viewer"))
    } else {
        var("XDG_CACHE_HOME")
            .or_else(|| var("HOME").map(|h| h.join(".cache")))
            .map(|d| d.join("moonglow-viewer"))
    };
    base.map(|d| d.join("shaders"))
}

impl Viewport {
    pub fn new(gpu: &Gpu) -> Viewport {
        let mut renderer = Renderer::new(gpu, FORMAT, SAMPLES);
        renderer.game_shaders = crate::GAME_SHADERS_BY_DEFAULT;
        if let Some(Some(dir)) = SHADER_CACHE.get() {
            renderer.set_shader_cache(Some(dir.clone()));
        }
        Viewport { renderer, overlays: OverlayPass::new(gpu), targets: None }
    }

    /// What the last frame drew (how many meshes the game's programs drew,
    /// how many wait for theirs).
    pub fn drawn(&self) -> mg_render::Drawn {
        self.renderer.drawn
    }

    /// The game's programs that could not be drawn, as `vs/fs` pairs or
    /// programs, and why (drawn with Moonglow's own shader instead).
    pub fn shader_failures(&self) -> &[(String, String)] {
        self.renderer.game_shader_failures()
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
