//! Output mapping: draws each screen of an [`OutputSetup`] from the composition.
//!
//! Every slice is a warp mesh ([`Slice::mesh`]) drawn into its screen's target, sampling the
//! composition output. Masks are rasterised on the CPU into 8-bit textures when the setup changes,
//! so a frame costs one draw per slice.

use bytemuck::{Pod, Zeroable};
use openvj_core::{OutputSetup, Screen, Slice};
use wgpu::util::DeviceExt;

use crate::{LDR, Target};

/// Mesh resolution of a slice; enough for smooth bezier warps and steep corner pins.
const MESH_CELLS: u32 = 32;
const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 2],
    src: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
struct Uniforms {
    sizes: [f32; 4],
    look: [f32; 4],
    rgb: [f32; 4],
    card: [f32; 4],
}

struct BuiltSlice {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    uniforms: wgpu::Buffer,
    /// Test card on or off, as last written to `uniforms`.
    card: bool,
    mask: Option<wgpu::TextureView>,
}

struct BuiltScreen {
    target: Target,
    mask: Option<wgpu::TextureView>,
    slices: Vec<BuiltSlice>,
}

/// GPU state of the output mapping.
pub(crate) struct Mapper {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    white: wgpu::TextureView,
    /// The setup and composition size the screens were built for.
    built_for: Option<(OutputSetup, [u32; 2])>,
    screens: Vec<BuiltScreen>,
}

impl Mapper {
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("openvj mapping"),
            source: wgpu::ShaderSource::Wgsl(include_str!("mapping.wgsl").into()),
        });
        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("openvj mapping layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(size_of::<Uniforms>() as u64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                tex(2),
                tex(3),
                tex(4),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("openvj mapping pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("openvj mapping"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_map"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                })],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_map"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: LDR,
                    // Later slices draw over earlier ones, faded by their masks; the screen stays opaque.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let white = mask_texture(device, queue, "mask none", 1, 1, &[255]);
        Self { pipeline, layout, white, built_for: None, screens: Vec::new() }
    }

    /// The target of screen `index`, once [`Mapper::render`] has drawn it.
    pub(crate) fn screen(&self, index: usize) -> Option<&Target> {
        self.screens.get(index).map(|s| &s.target)
    }

    /// Draws every enabled screen of `setup` from `comp`, rebuilding meshes and masks when the setup changed.
    pub(crate) fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sampler: &wgpu::Sampler,
        setup: &OutputSetup,
        comp: &Target,
        test_card: bool,
    ) {
        let comp_size = [comp.width, comp.height];
        if self.built_for.as_ref().is_none_or(|(s, c)| s != setup || *c != comp_size) {
            self.build(device, queue, setup, comp_size);
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("mapping") });
        for (screen, built) in setup.screens.iter().zip(&mut self.screens) {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("screen"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &built.target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if !screen.enabled {
                continue;
            }
            rp.set_pipeline(&self.pipeline);
            let screen_mask = built.mask.as_ref().unwrap_or(&self.white);
            for (slice, b) in screen.slices().filter(|s| s.enabled).zip(&mut built.slices) {
                if b.card != test_card {
                    b.card = test_card;
                    let u = uniforms(screen, slice, &built.target, comp_size, test_card);
                    queue.write_buffer(&b.uniforms, 0, bytemuck::bytes_of(&u));
                }
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: b.uniforms.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&comp.view) },
                        wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(screen_mask) },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(b.mask.as_ref().unwrap_or(&self.white)),
                        },
                    ],
                });
                rp.set_bind_group(0, &bind, &[]);
                rp.set_vertex_buffer(0, b.vertices.slice(..));
                rp.set_index_buffer(b.indices.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(0..b.count, 0, 0..1);
            }
        }
        queue.submit([encoder.finish()]);
    }

    fn build(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, setup: &OutputSetup, comp_size: [u32; 2]) {
        // Slices' inputs were laid out for the setup's composition size; follow the current one.
        let mut setup_scaled = setup.clone();
        setup_scaled.rescale_inputs(comp_size[0], comp_size[1]);
        let old = std::mem::take(&mut self.screens);
        let mut old = old.into_iter();
        for screen in &setup_scaled.screens {
            let [w, h] = screen.size().map(|v| v.clamp(16, 8192));
            // Keep the screen's texture when its size didn't change, so the UI's handle stays valid.
            let target = match old.next() {
                Some(b) if (b.target.width, b.target.height) == (w, h) => b.target,
                _ => Target::new(device, &screen.name, w, h, LDR),
            };
            let mask = ((w, h) == (screen.size()[0], screen.size()[1]))
                .then(|| screen.mask_image())
                .flatten()
                .map(|img| mask_texture(device, queue, "screen mask", screen.size()[0], screen.size()[1], &img));
            let slices = screen
                .slices()
                .filter(|s| s.enabled)
                .map(|slice| {
                    let mesh = slice.mesh(MESH_CELLS, MESH_CELLS);
                    let vertices: Vec<Vertex> =
                        mesh.vertices.iter().map(|v| Vertex { pos: v.position, src: v.uv }).collect();
                    let u = uniforms(screen, slice, &target, comp_size, false);
                    BuiltSlice {
                        vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("slice vertices"),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        }),
                        indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("slice indices"),
                            contents: bytemuck::cast_slice(&mesh.indices),
                            usage: wgpu::BufferUsages::INDEX,
                        }),
                        count: mesh.indices.len() as u32,
                        uniforms: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("slice uniforms"),
                            contents: bytemuck::bytes_of(&u),
                            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        }),
                        card: false,
                        mask: slice
                            .mask_image(comp_size)
                            .map(|img| mask_texture(device, queue, "slice mask", comp_size[0], comp_size[1], &img)),
                    }
                })
                .collect();
            self.screens.push(BuiltScreen { target, mask, slices });
        }
        self.built_for = Some((setup.clone(), comp_size));
    }
}

fn uniforms(screen: &Screen, slice: &Slice, target: &Target, comp: [u32; 2], card: bool) -> Uniforms {
    let (a, b) = (screen.color, slice.color);
    // A distinct, steady colour per slice for the test card.
    let hue = (slice.id as f32 * 0.618_034).fract();
    Uniforms {
        sizes: [target.width as f32, target.height as f32, comp[0] as f32, comp[1] as f32],
        look: [
            screen.opacity.clamp(0.0, 1.0),
            if card { 1.0 } else { 0.0 },
            a.brightness + b.brightness,
            a.contrast + b.contrast,
        ],
        rgb: [a.red + b.red, a.green + b.green, a.blue + b.blue, 0.0],
        card: [hue_rgb(hue)[0], hue_rgb(hue)[1], hue_rgb(hue)[2], 1.0],
    }
}

fn hue_rgb(h: f32) -> [f32; 3] {
    let f = |n: f32| {
        let k = (n + h * 6.0) % 6.0;
        1.0 - (k.min(4.0 - k).clamp(0.0, 1.0))
    };
    [f(5.0), f(3.0), f(1.0)]
}

fn mask_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    w: u32,
    h: u32,
    data: &[u8],
) -> wgpu::TextureView {
    device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: MASK_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            data,
        )
        .create_view(&Default::default())
}
