//! Overlays drawn over a stage's frame, in its own targets: lines and
//! translucent triangles in world space (walkmeshes, wireframes, normals,
//! the grid, the skeleton, the selection), tested against the scene's
//! depths, which the renderer keeps. Where the scene hides them they show
//! faintly, so a walkmesh under a floor or bones inside a body are still
//! seen; x-ray lines show fully over everything.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use mg_render::{Camera, DEPTH_FORMAT, Gpu, Targets};
use wgpu::util::DeviceExt;

use crate::render::{FORMAT, SAMPLES};

/// An overlay vertex: world position and colour (gamma space) with alpha.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct OverlayVertex {
    pub pos: [f32; 3],
    pub color: [f32; 4],
}

impl OverlayVertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<OverlayVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
    };
}

/// What to draw over a frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overlay {
    /// Segments (two vertices each), tested against the scene.
    pub lines: Vec<OverlayVertex>,
    /// Triangles (three vertices each), tested against the scene.
    pub triangles: Vec<OverlayVertex>,
    /// Segments over everything.
    pub xray: Vec<OverlayVertex>,
    /// Segments only where the scene does not hide them (the ground grid
    /// and axes: hidden behind models, as in a modelling program).
    pub culled: Vec<OverlayVertex>,
}

fn vertex(p: Vec3, color: [f32; 4]) -> OverlayVertex {
    OverlayVertex { pos: p.to_array(), color }
}

impl Overlay {
    pub fn line(&mut self, a: Vec3, b: Vec3, color: [f32; 4]) {
        self.lines.extend([vertex(a, color), vertex(b, color)]);
    }

    pub fn xray_line(&mut self, a: Vec3, b: Vec3, color: [f32; 4]) {
        self.xray.extend([vertex(a, color), vertex(b, color)]);
    }

    pub fn culled_line(&mut self, a: Vec3, b: Vec3, color: [f32; 4]) {
        self.culled.extend([vertex(a, color), vertex(b, color)]);
    }

    pub fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, color: [f32; 4]) {
        self.triangles.extend([vertex(a, color), vertex(b, color), vertex(c, color)]);
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
            && self.triangles.is_empty()
            && self.xray.is_empty()
            && self.culled.is_empty()
    }
}

/// How much of an overlay shows where the scene hides it.
pub const HIDDEN_ALPHA: f32 = 0.3;

/// How far overlays are drawn towards the eye (a fraction of their
/// distance), to lie on the surfaces they outline.
const PULL: f32 = 0.002;

const SHADER: &str = r"
struct Frame {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    // alpha scale, pull towards the eye
    params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> frame: Frame;

struct In {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
};
struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs(v: In) -> Out {
    var o: Out;
    let p = frame.view * vec4<f32>(v.pos, 1.0);
    o.clip = frame.proj * vec4<f32>(p.xyz * (1.0 - frame.params.y), 1.0);
    o.color = vec4<f32>(v.color.rgb, v.color.a * frame.params.x);
    return o;
}

@fragment
fn fs(o: Out) -> @location(0) vec4<f32> {
    return o.color;
}
";

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct FrameUniform {
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
    params: [f32; 4],
}

/// The pipelines that draw overlays into a stage's targets.
#[derive(Debug)]
pub struct OverlayPass {
    layout: wgpu::BindGroupLayout,
    /// Lines where the scene shows them, where it hides them, everywhere.
    lines: [wgpu::RenderPipeline; 3],
    /// Triangles where the scene shows them and where it hides them.
    triangles: [wgpu::RenderPipeline; 2],
}

impl OverlayPass {
    pub fn new(gpu: &Gpu) -> OverlayPass {
        let device = &gpu.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("overlay"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("overlay"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("overlay"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |topology, compare| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("overlay"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(OverlayVertex::LAYOUT)],
                },
                primitive: wgpu::PrimitiveState { topology, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: FORMAT,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        use wgpu::CompareFunction::{Always, Greater, LessEqual};
        use wgpu::PrimitiveTopology::{LineList, TriangleList};
        OverlayPass {
            layout,
            lines: [
                pipeline(LineList, LessEqual),
                pipeline(LineList, Greater),
                pipeline(LineList, Always),
            ],
            triangles: [pipeline(TriangleList, LessEqual), pipeline(TriangleList, Greater)],
        }
    }

    /// Draws an overlay over what `targets` hold (drawn with `camera`),
    /// resolving them again.
    pub fn draw(&self, gpu: &Gpu, overlay: &Overlay, camera: &Camera, targets: &Targets) {
        if overlay.is_empty() {
            return;
        }
        let device = &gpu.device;
        let (w, h) = targets.size;
        let proj = camera.projection(w as f32 / h.max(1) as f32);
        let group = |alpha: f32| {
            let uniform = FrameUniform {
                view: camera.view().to_cols_array_2d(),
                proj: proj.to_cols_array_2d(),
                params: [alpha, PULL, 0.0, 0.0],
            };
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("overlay"),
                contents: bytemuck::bytes_of(&uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("overlay"),
                layout: &self.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
        };
        let (shown, hidden) = (group(1.0), group(HIDDEN_ALPHA));
        let vertices: Vec<OverlayVertex> =
            [&overlay.triangles, &overlay.lines, &overlay.xray, &overlay.culled]
                .into_iter()
                .flatten()
                .copied()
                .collect();
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("overlay"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let t = overlay.triangles.len() as u32;
        let l = t + overlay.lines.len() as u32;
        let x = l + overlay.xray.len() as u32;
        let c = x + overlay.culled.len() as u32;
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: targets.render_view(),
                    resolve_target: targets.resolve_view(),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_vertex_buffer(0, buffer.slice(..));
            // Hidden parts first, faint; then what shows; then x-ray.
            pass.set_bind_group(0, &hidden, &[]);
            if t > 0 {
                pass.set_pipeline(&self.triangles[1]);
                pass.draw(0..t, 0..1);
            }
            if l > t {
                pass.set_pipeline(&self.lines[1]);
                pass.draw(t..l, 0..1);
            }
            pass.set_bind_group(0, &shown, &[]);
            if c > x {
                pass.set_pipeline(&self.lines[0]);
                pass.draw(x..c, 0..1);
            }
            if t > 0 {
                pass.set_pipeline(&self.triangles[0]);
                pass.draw(0..t, 0..1);
            }
            if l > t {
                pass.set_pipeline(&self.lines[0]);
                pass.draw(t..l, 0..1);
            }
            if x > l {
                pass.set_pipeline(&self.lines[2]);
                pass.draw(l..x, 0..1);
            }
        }
        gpu.queue.submit([encoder.finish()]);
    }
}
