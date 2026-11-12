//! openVJ's GPU engine.
//!
//! Each frame is built as a list of fullscreen passes
//! (clip → layer effects → blend → composition effects → master),
//! then encoded and submitted in one command buffer.
//!
//! Mixing and effects run in 16-bit float (`Rgba16Float`) so feedback effects and soft
//! gradients don't band. The targets the UI shows (output, preview, thumbnails) are
//! `Rgba8Unorm` with `TEXTURE_BINDING | RENDER_ATTACHMENT`, which Slint imports without copies.
//!
//! Effects are ISF files (see `openvj_core::isf`), compiled on first use by [`isf`].

mod isf;
mod mapping;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use bytemuck::{Pod, Zeroable};
use openvj_core::isf::eval_size;
use openvj_core::{Clip, ClipId, ClipSource, Composition, Effect, EffectType, ModInputs, OutputSetup};

use crate::isf::CompiledEffect;

pub use wgpu;

/// Checks that an effect's GLSL compiles, without a GPU. Effect folders use it so broken
/// files show their error in the browser before anyone adds them.
pub fn check_effect(def: &openvj_core::isf::EffectDef) -> Result<(), String> {
    isf::parse(def).map(|_| ())
}

/// Size of live clip thumbnails in the grid.
pub const THUMB_SIZE: (u32, u32) = (192, 108);
/// Seconds between live thumbnail refreshes (15 fps). They're small grid previews, so
/// redrawing them at full frame rate would spend GPU time nobody sees.
const THUMB_INTERVAL: f64 = 1.0 / 15.0;
/// Format of everything the UI displays.
const LDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Format of the mixing and effect targets.
const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Format of [`Engine::shared_output`], the one Syphon (and most sharing APIs) expects.
const SHARE: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
const MODE_MASTER: u32 = 100;
const MODE_MIX: u32 = 101;
/// Per-pass uniform slot size; covers `min_uniform_buffer_offset_alignment` everywhere.
const UNIFORM_STRIDE: u64 = 256;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
struct Uniforms {
    res: [f32; 2],
    time: f32,
    beat: f32,
    params: [f32; 4],
    kind: u32,
    mode: u32,
    opacity: f32,
    _pad: f32,
    uv_scale: [f32; 2],
    uv_offset: [f32; 2],
    /// Media passes: the part of tex_a (xy) and tex_b (zw) holding the picture.
    crop: [f32; 4],
}

/// A texture the engine renders into.
#[derive(Clone)]
pub struct Target {
    pub texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    id: u64,
}

impl Target {
    fn new(device: &wgpu::Device, label: &str, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self { texture, view, width, height, format, id: NEXT.fetch_add(1, Ordering::Relaxed) }
    }
}

/// How a block-compressed media frame is stored; see [`Engine::set_media_bc`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BcLayout {
    /// BC1, always opaque (DXV normal quality, HAP).
    Bc1Opaque,
    /// BC3 with premultiplied alpha (DXV normal quality with alpha).
    Bc3Premultiplied,
    /// BC3 with straight alpha (HAP Alpha).
    Bc3,
    /// BC7 with straight alpha (HAP R).
    Bc7,
    /// BC4 luma plus half-size BC5 Co/Cg (DXV high quality).
    YCoCg,
    /// BC5 luma/alpha plus half-size BC5 Co/Cg (DXV high quality with alpha).
    YCoCgAlpha,
    /// BC3 holding scaled YCoCg: R = Co, G = Cg, B = scale, A = luma (HAP Q).
    ScaledYCoCg,
    /// Scaled YCoCg BC3 plus a full-size BC4 alpha texture (HAP Q Alpha).
    ScaledYCoCgAlpha,
}

impl BcLayout {
    fn formats(self) -> (wgpu::TextureFormat, Option<wgpu::TextureFormat>) {
        use wgpu::TextureFormat as F;
        match self {
            BcLayout::Bc1Opaque => (F::Bc1RgbaUnorm, None),
            BcLayout::Bc3Premultiplied | BcLayout::Bc3 | BcLayout::ScaledYCoCg => (F::Bc3RgbaUnorm, None),
            BcLayout::Bc7 => (F::Bc7RgbaUnorm, None),
            BcLayout::YCoCg => (F::Bc4RUnorm, Some(F::Bc5RgUnorm)),
            BcLayout::YCoCgAlpha => (F::Bc5RgUnorm, Some(F::Bc5RgUnorm)),
            BcLayout::ScaledYCoCgAlpha => (F::Bc3RgbaUnorm, Some(F::Bc4RUnorm)),
        }
    }

    /// How many times smaller the second texture is than the first, in each direction.
    fn second_divisor(self) -> u32 {
        match self {
            BcLayout::YCoCg | BcLayout::YCoCgAlpha => 2,
            _ => 1,
        }
    }

    /// `u.mode` of `fs_media` (the MEDIA_* constants in shaders.wgsl).
    fn shader_mode(self) -> u32 {
        match self {
            BcLayout::Bc3 | BcLayout::Bc7 => 0,
            BcLayout::Bc1Opaque => 1,
            BcLayout::Bc3Premultiplied => 2,
            BcLayout::YCoCg => 3,
            BcLayout::YCoCgAlpha => 4,
            BcLayout::ScaledYCoCg => 5,
            BcLayout::ScaledYCoCgAlpha => 6,
        }
    }
}

/// A block-compressed frame, uploaded as-is and expanded by the GPU when sampled.
#[derive(Debug, Clone, Copy)]
pub struct BcFrame<'a> {
    pub layout: BcLayout,
    /// Visible size.
    pub width: u32,
    pub height: u32,
    /// Size of `data` in pixels, multiples of 4 and at least the visible size.
    pub coded_width: u32,
    pub coded_height: u32,
    /// Blocks in row-major order.
    pub data: &'a [u8],
    /// The second texture's blocks, for layouts that have one: BC5 Co/Cg at half width
    /// (DXV high quality) or BC4 alpha at full width (HAP Q Alpha).
    pub chroma: &'a [u8],
    /// Height of the second texture in pixels.
    pub chroma_height: u32,
}

/// What a media texture was created for; a frame with another shape needs new textures.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MediaShape {
    Rgba(u32, u32),
    Bc(BcLayout, u32, u32, u32),
}

struct MediaTexture {
    shape: MediaShape,
    textures: Vec<wgpu::Texture>,
    view: wgpu::TextureView,
    chroma: Option<wgpu::TextureView>,
    /// Visible size.
    width: u32,
    height: u32,
    mode: u32,
    crop: [f32; 4],
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Pipeline {
    Generator,
    Media,
    Blend,
}

enum Draw {
    Clear,
    Builtin(Pipeline),
    Isf {
        fx: Rc<CompiledEffect>,
        /// (binding, packed bytes) per uniform block.
        blocks: Vec<(u32, Vec<u8>)>,
        /// (binding, view) per texture.
        textures: Vec<(u32, wgpu::TextureView)>,
    },
}

/// One fullscreen draw, or just a clear.
struct Pass {
    draw: Draw,
    target: wgpu::TextureView,
    format: wgpu::TextureFormat,
    clear: wgpu::Color,
    uniforms: Uniforms,
    a: Option<wgpu::TextureView>,
    b: Option<wgpu::TextureView>,
}

impl Pass {
    fn clear(target: &Target, color: wgpu::Color) -> Self {
        Self::builtin(None, target, color, Uniforms::default(), None, None)
    }

    fn builtin(
        pipeline: Option<Pipeline>,
        target: &Target,
        clear: wgpu::Color,
        uniforms: Uniforms,
        a: Option<wgpu::TextureView>,
        b: Option<wgpu::TextureView>,
    ) -> Self {
        Self {
            draw: pipeline.map_or(Draw::Clear, Draw::Builtin),
            target: target.view.clone(),
            format: target.format,
            clear,
            uniforms,
            a,
            b,
        }
    }
}

#[derive(Clone)]
struct Targets {
    clip_a: Target,
    clip_b: Target,
    layer_mix: Target,
    /// Scratch targets for effect chains; three so an effect's dry input, its wet output
    /// and the dry/wet mix never share one.
    fx: [Target; 3],
    /// Ping-pong pair a layer group is mixed in before it joins the composition.
    group: [Target; 2],
    comp: [Target; 2],
    output: Target,
    preview: Target,
}

impl Targets {
    fn new(device: &wgpu::Device, w: u32, h: u32) -> Self {
        let hdr = |label| Target::new(device, label, w, h, HDR);
        Self {
            clip_a: hdr("clip a"),
            clip_b: hdr("clip b"),
            layer_mix: hdr("layer mix"),
            fx: [hdr("fx 0"), hdr("fx 1"), hdr("fx 2")],
            group: [hdr("group 0"), hdr("group 1")],
            comp: [hdr("comp 0"), hdr("comp 1")],
            output: Target::new(device, "output", w, h, LDR),
            preview: Target::new(device, "preview", w, h, LDR),
        }
    }
}

/// A pass target of one effect instance, double-buffered so a pass can read the previous
/// frame's contents while writing new ones.
struct FxBuffer {
    pair: [Target; 2],
    /// Index of the side holding the latest contents.
    latest: usize,
}

/// GPU state of one effect instance (one [`Effect::uid`]).
#[derive(Default)]
struct FxState {
    /// [`EffectType::version`] the buffers were made for.
    version: u64,
    buffers: HashMap<String, FxBuffer>,
    frame: u32,
    last_time: Option<f64>,
}

pub struct Engine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    shader: wgpu::ShaderModule,
    pipelines: HashMap<(Pipeline, wgpu::TextureFormat), wgpu::RenderPipeline>,
    uniform_buf: wgpu::Buffer,
    uniform_slots: u64,
    fx_uniform_buf: wgpu::Buffer,
    dummy: wgpu::TextureView,
    targets: Targets,
    thumbs: HashMap<ClipId, Target>,
    media: HashMap<ClipId, MediaTexture>,
    /// By effect id: the [`EffectType::version`] compiled and the result.
    compiled: HashMap<String, (u64, Result<Rc<CompiledEffect>, String>)>,
    fx_state: HashMap<u64, FxState>,
    /// Effect instances that ran this frame.
    fx_seen: HashSet<u64>,
    /// A transparent 1×1 texture: the `inputImage` of ISF generators.
    blank: Target,
    /// HDR targets by size, for drawing ISF generators meant for LDR targets.
    hdr_stand_ins: HashMap<(u32, u32), Target>,
    mapper: mapping::Mapper,
    /// A BGRA copy of the output for other apps, while one is wanted.
    shared: Option<Target>,
    /// When live thumbnails were last drawn, on the composition clock.
    thumbs_at: Option<f64>,
    /// Passes in the last frame; tests use it to catch work creeping into every frame.
    last_passes: usize,
    /// Whether this frame's preview was already drawn from a clip on screen.
    previewed: bool,
}

const TRANSPARENT: wgpu::Color = wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

impl Engine {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, width: u32, height: u32) -> Self {
        let blank = Target::new(&device, "blank", 1, 1, HDR);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("openvj shaders"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders.wgsl").into()),
        });
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
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
            label: Some("openvj bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
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
                tex_entry(2),
                tex_entry(3),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("openvj pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let make = |entry: &str, format| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let mut pipelines = HashMap::new();
        for format in [LDR, HDR] {
            pipelines.insert((Pipeline::Generator, format), make("fs_generator", format));
            pipelines.insert((Pipeline::Media, format), make("fs_media", format));
            pipelines.insert((Pipeline::Blend, format), make("fs_blend", format));
        }
        pipelines.insert((Pipeline::Blend, SHARE), make("fs_blend", SHARE));

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("openvj sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let dummy = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("dummy"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: LDR,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let uniform_slots = 64;
        let uniform_buf = Self::create_uniform_buffer(&device, uniform_slots * UNIFORM_STRIDE);
        let fx_uniform_buf = Self::create_uniform_buffer(&device, 64 * UNIFORM_STRIDE);
        let targets = Targets::new(&device, width, height);
        let mapper = mapping::Mapper::new(&device, &queue);
        Self {
            device,
            queue,
            layout,
            sampler,
            shader,
            pipelines,
            uniform_buf,
            uniform_slots,
            fx_uniform_buf,
            dummy,
            targets,
            thumbs: HashMap::new(),
            media: HashMap::new(),
            compiled: HashMap::new(),
            fx_state: HashMap::new(),
            fx_seen: HashSet::new(),
            blank,
            hdr_stand_ins: HashMap::new(),
            mapper,
            shared: None,
            thumbs_at: None,
            last_passes: 0,
            previewed: false,
        }
    }

    fn create_uniform_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("openvj uniforms"),
            size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn size(&self) -> (u32, u32) {
        (self.targets.output.width, self.targets.output.height)
    }

    /// Recreates the composition-sized targets. Textures from [`Engine::output`] and
    /// [`Engine::preview`] must be fetched again afterwards.
    pub fn resize(&mut self, width: u32, height: u32) {
        if (width, height) != self.size() {
            self.targets = Targets::new(&self.device, width.max(16), height.max(16));
            self.fx_state.clear();
            if self.shared.is_some() {
                self.shared = None;
                self.set_shared_output(true);
            }
        }
    }

    /// Turns the shared output on or off. While on, every frame also copies the output
    /// into a `Bgra8Unorm` texture for other apps (Syphon), one fullscreen pass.
    pub fn set_shared_output(&mut self, on: bool) {
        match (on, self.shared.is_some()) {
            (true, false) => {
                let (w, h) = self.size();
                self.shared = Some(Target::new(&self.device, "shared output", w, h, SHARE));
            }
            (false, true) => self.shared = None,
            _ => {}
        }
    }

    /// The BGRA copy of the last frame's output, while [`Engine::set_shared_output`] is on.
    pub fn shared_output(&self) -> Option<&Target> {
        self.shared.as_ref()
    }

    /// The final mix.
    pub fn output(&self) -> &Target {
        &self.targets.output
    }

    /// The clip passed to [`Engine::render`] as `preview`, rendered on its own.
    /// Draws every screen of `setup` from the output, through its slices, warps and masks.
    /// Call after [`Engine::render`]. With `test_card`, slices show a grid instead of the output.
    pub fn map_output(&mut self, setup: &OutputSetup, test_card: bool) {
        let comp = self.targets.output.clone();
        self.mapper.render(&self.device, &self.queue, &self.sampler, setup, &comp, test_card);
    }

    /// Screen `index` as last drawn by [`Engine::map_output`]. Its texture changes when the
    /// screen's size changes, so fetch it again after changing the setup.
    pub fn screen(&self, index: usize) -> Option<&Target> {
        self.mapper.screen(index)
    }

    pub fn preview(&self) -> &Target {
        &self.targets.preview
    }

    /// Live thumbnail for a generator or live input clip, created on first use.
    pub fn thumb(&mut self, id: ClipId) -> &Target {
        let device = &self.device;
        self.thumbs.entry(id).or_insert_with(|| Target::new(device, "thumb", THUMB_SIZE.0, THUMB_SIZE.1, LDR))
    }

    fn media_texture(&self, label: &str, format: wgpu::TextureFormat, width: u32, height: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    fn write_media(&self, texture: &wgpu::Texture, data: &[u8], bytes_per_row: u32, width: u32, height: u32) {
        self.queue.write_texture(
            texture.as_image_copy(),
            data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bytes_per_row), rows_per_image: None },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
    }

    /// Uploads an RGBA8 frame for a media clip.
    pub fn set_media_frame(&mut self, id: ClipId, width: u32, height: u32, rgba: &[u8]) {
        assert_eq!(rgba.len(), (width * height * 4) as usize, "frame size mismatch");
        let shape = MediaShape::Rgba(width, height);
        if self.media.get(&id).is_none_or(|m| m.shape != shape) {
            let texture = self.media_texture("media", LDR, width, height);
            let view = texture.create_view(&Default::default());
            let m = MediaTexture {
                shape,
                textures: vec![texture],
                view,
                chroma: None,
                width,
                height,
                mode: 0,
                crop: [1.0; 4],
            };
            self.media.insert(id, m);
        }
        self.write_media(&self.media[&id].textures[0], rgba, width * 4, width, height);
    }

    /// True if the GPU samples BC (DXT / RGTC) textures, so [`Engine::set_media_bc`] works.
    pub fn supports_bc(&self) -> bool {
        self.device.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
    }

    /// Uploads a block-compressed frame for a media clip. Requires [`Engine::supports_bc`].
    pub fn set_media_bc(&mut self, id: ClipId, frame: &BcFrame) {
        let BcFrame { layout, width, height, coded_width: cw, coded_height: ch, .. } = *frame;
        let (format, chroma_format) = layout.formats();
        let div = layout.second_divisor();
        let align = if chroma_format.is_some() { 4 * div } else { 4 };
        assert!(cw % align == 0 && ch % 4 == 0 && cw >= width && ch >= height, "bad BC frame size");
        let block_bytes = |f: wgpu::TextureFormat| f.block_copy_size(None).expect("BC format");
        let bpr = cw / 4 * block_bytes(format);
        assert_eq!(frame.data.len(), (bpr * ch / 4) as usize, "BC frame size mismatch");
        let (hw, hh) = (cw / div, frame.chroma_height);
        let chroma_bpr = chroma_format.map(|f| hw / 4 * block_bytes(f));
        if let Some(bpr) = chroma_bpr {
            assert!(hh % 4 == 0 && hh > 0, "bad chroma size");
            assert_eq!(frame.chroma.len(), (bpr * hh / 4) as usize, "BC chroma size mismatch");
        }

        let shape = MediaShape::Bc(layout, cw, ch, hh);
        if self.media.get(&id).is_none_or(|m| m.shape != shape) {
            let main = self.media_texture("media bc", format, cw, ch);
            let view = main.create_view(&Default::default());
            let mut textures = vec![main];
            let chroma = chroma_format.map(|f| {
                let t = self.media_texture("media chroma", f, hw, hh);
                let v = t.create_view(&Default::default());
                textures.push(t);
                v
            });
            // A half-size second texture's texels cover 2×2 pixels of the coded frame.
            let crop = [
                width as f32 / cw as f32,
                height as f32 / ch as f32,
                width as f32 / cw as f32,
                height as f32 / (div * hh.max(1)) as f32,
            ];
            let m = MediaTexture { shape, textures, view, chroma, width, height, mode: layout.shader_mode(), crop };
            self.media.insert(id, m);
        }
        let m = &self.media[&id];
        self.write_media(&m.textures[0], frame.data, bpr, cw, ch);
        if let Some(bpr) = chroma_bpr {
            self.write_media(&m.textures[1], frame.chroma, bpr, hw, hh);
        }
    }

    pub fn has_media(&self, id: ClipId) -> bool {
        self.media.contains_key(&id)
    }

    /// Frees GPU resources of clips that no longer exist.
    pub fn retain_clips(&mut self, alive: &HashSet<ClipId>) {
        self.thumbs.retain(|id, _| alive.contains(id));
        self.media.retain(|id, _| alive.contains(id));
    }

    /// Why an effect type can't run (its GLSL didn't compile), once it has been tried.
    pub fn effect_error(&self, kind: &str) -> Option<&str> {
        self.compiled.get(kind)?.1.as_ref().err().map(String::as_str)
    }

    /// The pipeline for an effect type, compiled on first use and again when its file changed.
    fn compiled(&mut self, ty: &EffectType) -> Option<Rc<CompiledEffect>> {
        if self.compiled.get(ty.id()).is_none_or(|(v, _)| *v != ty.version) {
            let result = isf::compile(&self.device, &self.shader, HDR, &ty.def).map(Rc::new);
            if let Err(e) = &result {
                eprintln!("effect {} failed to compile:\n{e}", ty.id());
            }
            self.compiled.insert(ty.id().to_owned(), (ty.version, result));
        }
        self.compiled[ty.id()].1.as_ref().ok().cloned()
    }

    fn clip_pass(&self, clip: &Clip, time: f64, inputs: &ModInputs, target: &Target) -> Pass {
        let mut u = Uniforms {
            res: [target.width as f32, target.height as f32],
            // Keep f32 precision for long sets.
            time: (time % 3600.0) as f32,
            beat: (inputs.beat % 1024.0) as f32,
            params: clip.param_values(inputs),
            uv_scale: [1.0, 1.0],
            ..Default::default()
        };
        match &clip.source {
            ClipSource::Generator { kind } => {
                u.kind = kind.shader_id();
                Pass::builtin(Some(Pipeline::Generator), target, TRANSPARENT, u, None, None)
            }
            // Files and live inputs both arrive as uploaded frames.
            _ => match self.media.get(&clip.id) {
                Some(m) => {
                    u.uv_scale = cover_scale(m.width, m.height, target.width, target.height);
                    u.mode = m.mode;
                    u.crop = m.crop;
                    Pass::builtin(Some(Pipeline::Media), target, TRANSPARENT, u, Some(m.view.clone()), m.chroma.clone())
                }
                None => Pass::clear(target, TRANSPARENT),
            },
        }
    }

    /// Draws a clip into `target`. `slot` tells apart the places one clip is drawn in a frame
    /// (layer, preview, thumbnail), so each keeps its own ISF feedback state.
    #[allow(clippy::too_many_arguments)]
    fn clip_passes(
        &mut self,
        passes: &mut Vec<Pass>,
        scratch: &[Target; 3],
        clip: &Clip,
        time: f64,
        inputs: &ModInputs,
        target: &Target,
        slot: u64,
    ) {
        let ClipSource::Isf { generator } = &clip.source else {
            passes.push(self.clip_pass(clip, time, inputs, target));
            return;
        };
        let key = generator.uid ^ (slot << 56);
        let blank = self.blank.clone();
        // ISF pipelines draw in HDR; LDR targets (preview, thumbnails) get a same-size HDR
        // stand-in and a copy.
        let dst = if target.format == HDR {
            target.clone()
        } else {
            let device = &self.device;
            let size = (target.width, target.height);
            self.hdr_stand_ins
                .entry(size)
                .or_insert_with(|| Target::new(device, "isf stand-in", size.0, size.1, HDR))
                .clone()
        };
        match self.isf_passes(passes, scratch, generator, key, &blank, Some(&dst), time, inputs) {
            Some(out) if out.id != target.id => passes.push(Self::blend_pass(target, &out, &out, MODE_MIX, 1.0)),
            Some(_) => {}
            None => passes.push(Pass::clear(target, TRANSPARENT)),
        }
    }

    fn blend_pass(target: &Target, a: &Target, b: &Target, mode: u32, opacity: f32) -> Pass {
        let u = Uniforms { res: [target.width as f32, target.height as f32], mode, opacity, ..Default::default() };
        Pass::builtin(Some(Pipeline::Blend), target, TRANSPARENT, u, Some(a.view.clone()), Some(b.view.clone()))
    }

    /// Runs `src` through the enabled effects of a chain and returns the target holding the
    /// result. Effects that are missing or don't compile pass their input through.
    fn effect_chain(
        &mut self,
        passes: &mut Vec<Pass>,
        scratch: &[Target; 3],
        mut src: Target,
        chain: &[Effect],
        now: f64,
        inputs: &ModInputs,
    ) -> Target {
        let free = |avoid: &[&Target]| scratch.iter().find(|t| avoid.iter().all(|a| a.id != t.id)).unwrap().clone();
        for fx in chain.iter().filter(|fx| fx.enabled) {
            let Some(out) = self.isf_passes(passes, scratch, fx, fx.uid, &src, None, now, inputs) else { continue };
            let mix = fx.mix.eval(inputs);
            src = if mix >= 0.999 {
                out
            } else {
                let dst = free(&[&src, &out]);
                passes.push(Self::blend_pass(&dst, &src, &out, MODE_MIX, mix));
                dst
            };
        }
        src
    }

    /// Records the passes of one ISF file run on `src` and returns the target holding its
    /// output: `dst` when given (the last pass draws straight into it), else a scratch target.
    /// `key` names the instance's feedback state. `None` when the file is missing or broken.
    #[allow(clippy::too_many_arguments)]
    fn isf_passes(
        &mut self,
        passes: &mut Vec<Pass>,
        scratch: &[Target; 3],
        fx: &Effect,
        key: u64,
        src: &Target,
        dst: Option<&Target>,
        time: f64,
        inputs: &ModInputs,
    ) -> Option<Target> {
        let free = |avoid: &[&Target]| scratch.iter().find(|t| avoid.iter().all(|a| a.id != t.id)).unwrap().clone();
        let ty = fx.ty()?;
        let compiled = self.compiled(&ty)?;
        self.fx_seen.insert(key);
        let (w, h) = dst.map_or((src.width, src.height), |d| (d.width, d.height));
        let state = self.fx_state.entry(key).or_default();
        if state.version != ty.version {
            // New instance or its file changed: start with fresh feedback buffers.
            *state = FxState { version: ty.version, ..Default::default() };
        }
        let delta = state.last_time.map_or(0.0, |t| (time - t).max(0.0));
        state.last_time = Some(time);

        let mut values = isf::Values::default();
        let norm = fx.values(inputs);
        for (input_index, input) in ty.def.inputs.iter().enumerate() {
            let v: Vec<f32> = ty
                .channels
                .iter()
                .zip(&norm)
                .filter(|(ch, _)| ch.input == input_index)
                .map(|(ch, v)| input.real(ch.component, *v))
                .collect();
            if !v.is_empty() {
                values.set(&input.name, &v);
            }
        }
        let bands = inputs.bands;
        values.set("TIME", &[(time % 3600.0) as f32]);
        values.set("TIMEDELTA", &[delta as f32]);
        values.set("FRAMEINDEX", &[state.frame as f32]);
        values.set("BEAT", &[(inputs.beat % 1024.0) as f32]);
        values.set("AUDIO", &bands);
        values.set("audio_level", &[bands.iter().sum::<f32>() / 4.0]);
        values.set("audio_bass", &[bands[0]]);
        values.set("audio_mid", &[(bands[1] + bands[2]) * 0.5]);
        values.set("audio_treble", &[bands[3]]);
        values.set("audio_beat_phase", &[inputs.beat.rem_euclid(1.0) as f32]);
        state.frame = state.frame.wrapping_add(1);

        let last = ty.def.passes.len() - 1;
        let mut out = src.clone();
        for (i, pass) in ty.def.passes.iter().enumerate() {
            let size = |expr: &str, full: u32| {
                eval_size(expr, w as f32, h as f32).map_or(full, |v| v.round().clamp(1.0, 8192.0) as u32)
            };
            let (pw, ph) = (size(&pass.width, w), size(&pass.height, h));
            let target = match &pass.target {
                Some(name) => {
                    let device = &self.device;
                    let buf = state.buffers.entry(name.clone()).or_insert_with(|| FxBuffer {
                        pair: [Target::new(device, name, pw, ph, HDR), Target::new(device, name, pw, ph, HDR)],
                        latest: 0,
                    });
                    if (buf.pair[0].width, buf.pair[0].height) != (pw, ph) {
                        *buf = FxBuffer {
                            pair: [Target::new(device, name, pw, ph, HDR), Target::new(device, name, pw, ph, HDR)],
                            latest: 0,
                        };
                    }
                    buf.pair[1 - buf.latest].clone()
                }
                None => match dst {
                    Some(d) if i == last => d.clone(),
                    _ => free(&[src]),
                },
            };
            values.set("PASSINDEX", &[i as f32]);
            values.set("RENDERSIZE", &[target.width as f32, target.height as f32]);
            let blocks = compiled
                .blocks
                .iter()
                .map(|b| {
                    let mut bytes = vec![0u8; b.size as usize];
                    values.pack(b, &mut bytes);
                    (b.binding, bytes)
                })
                .collect();
            let textures = compiled
                .textures
                .iter()
                .filter_map(|(binding, name)| {
                    let view = if name == "inputImage" {
                        src.view.clone()
                    } else {
                        let buf = state.buffers.get(name)?;
                        buf.pair[buf.latest].view.clone()
                    };
                    Some((*binding, view))
                })
                .collect();
            passes.push(Pass {
                draw: Draw::Isf { fx: compiled.clone(), blocks, textures },
                target: target.view.clone(),
                format: target.format,
                clear: TRANSPARENT,
                uniforms: Uniforms::default(),
                a: None,
                b: None,
            });
            if let Some(name) = &pass.target {
                let buf = state.buffers.get_mut(name).expect("created above");
                buf.latest = 1 - buf.latest;
            }
            if i == last {
                out = target;
            }
        }
        Some(out)
    }

    /// Draws a clip into `target` through its Transform and effects, and copies the result
    /// into the preview when it is the previewed clip. `slot` is as for `clip_passes`.
    #[allow(clippy::too_many_arguments)]
    fn draw_clip(
        &mut self,
        passes: &mut Vec<Pass>,
        t: &Targets,
        clip: &Clip,
        time: f64,
        target: &Target,
        now: f64,
        inputs: &ModInputs,
        preview: Option<ClipId>,
        slot: u64,
    ) {
        self.clip_passes(passes, &t.fx, clip, time, inputs, target, slot);
        if clip_has_fx(clip) {
            let mut src = target.clone();
            if !clip.transform.is_noop() {
                src = self.effect_chain(passes, &t.fx, src, std::slice::from_ref(&clip.transform), now, inputs);
            }
            src = self.effect_chain(passes, &t.fx, src, &clip.effects, now, inputs);
            if src.id != target.id {
                passes.push(Self::blend_pass(target, &src, &src, MODE_MIX, 0.0));
            }
        }
        if preview == Some(clip.id) && !self.previewed {
            passes.push(Self::blend_pass(&t.preview, target, target, MODE_MIX, 0.0));
            self.previewed = true;
        }
    }

    /// Runs a finished group mix through the group's effects and blends it into the
    /// composition. Returns the composition's new ping-pong slot.
    #[allow(clippy::too_many_arguments)]
    fn close_group(
        &mut self,
        passes: &mut Vec<Pass>,
        t: &Targets,
        comp: &Composition,
        group: usize,
        slot: usize,
        cur: usize,
        now: f64,
        inputs: &ModInputs,
    ) -> usize {
        let Some(g) = comp.groups.get(group) else { return cur };
        let src = self.effect_chain(passes, &t.fx, t.group[slot].clone(), &g.effects, now, inputs);
        passes.push(Self::blend_pass(
            &t.comp[1 - cur],
            &t.comp[cur],
            &src,
            g.blend.shader_id(),
            g.opacity_value(inputs),
        ));
        1 - cur
    }

    /// Renders one frame.
    ///
    /// * `now`: composition clock in seconds (the same clock used to trigger clips).
    /// * `inputs`: beat position and audio levels that drive modulated parameters.
    /// * `preview`: a clip to render on its own into [`Engine::preview`].
    ///
    /// Every generator and live input clip in `comp` also refreshes its live thumbnail,
    /// at most every [`THUMB_INTERVAL`] (and right away when one is new).
    pub fn render(&mut self, comp: &Composition, now: f64, inputs: &ModInputs, preview: Option<ClipId>) {
        let thumbs_before = self.thumbs.len();
        for clip in comp.all_clips() {
            if clip.source.is_generator() || clip.source.is_live() {
                self.thumb(clip.id);
            }
        }
        let refresh_thumbs =
            self.thumbs.len() != thumbs_before || self.thumbs_at.is_none_or(|t| now < t || now - t >= THUMB_INTERVAL);
        if refresh_thumbs {
            self.thumbs_at = Some(now);
        }

        let t = self.targets.clone();
        self.fx_seen.clear();
        let beat = inputs.beat;
        let mut passes = vec![Pass::clear(&t.comp[0], wgpu::Color::BLACK)];
        self.previewed = false;
        let mut cur = 0;
        // The group being mixed (index, ping-pong slot), if any. A group's layers are mixed
        // on their own, from transparent, then blended into the composition as one.
        let mut open: Option<(usize, usize)> = None;
        for (_, layer) in comp.visible_layers() {
            let Some(frame) = layer.frame(now, beat) else { continue };
            if open.map(|(g, _)| g) != layer.group {
                if let Some((g, slot)) = open.take() {
                    cur = self.close_group(&mut passes, &t, comp, g, slot, cur, now, inputs);
                }
                if let Some(g) = layer.group {
                    passes.push(Pass::clear(&t.group[0], TRANSPARENT));
                    open = Some((g, 0));
                }
            }
            let layer_opacity = comp.layer_opacity(layer, inputs);
            let (src, opacity) = match (frame.incoming, frame.outgoing) {
                (Some((clip, time)), None) => {
                    self.draw_clip(&mut passes, &t, clip, time, &t.clip_a, now, inputs, preview, 0);
                    (&t.clip_a, layer_opacity * frame.mix)
                }
                (None, Some((clip, time))) => {
                    self.draw_clip(&mut passes, &t, clip, time, &t.clip_a, now, inputs, preview, 0);
                    (&t.clip_a, layer_opacity * (1.0 - frame.mix))
                }
                (Some((inc, inc_t)), Some((out, out_t))) => {
                    self.draw_clip(&mut passes, &t, out, out_t, &t.clip_a, now, inputs, preview, 0);
                    self.draw_clip(&mut passes, &t, inc, inc_t, &t.clip_b, now, inputs, preview, 0);
                    passes.push(Self::blend_pass(&t.layer_mix, &t.clip_a, &t.clip_b, MODE_MIX, frame.mix));
                    (&t.layer_mix, layer_opacity)
                }
                (None, None) => continue,
            };
            let mut src = src.clone();
            if !layer.transform.is_noop() {
                src = self.effect_chain(&mut passes, &t.fx, src, std::slice::from_ref(&layer.transform), now, inputs);
            }
            let src = &self.effect_chain(&mut passes, &t.fx, src, &layer.effects, now, inputs);
            let mode = layer.blend.shader_id();
            match &mut open {
                Some((_, slot)) => {
                    passes.push(Self::blend_pass(&t.group[1 - *slot], &t.group[*slot], src, mode, opacity));
                    *slot = 1 - *slot;
                }
                None => {
                    passes.push(Self::blend_pass(&t.comp[1 - cur], &t.comp[cur], src, mode, opacity));
                    cur = 1 - cur;
                }
            }
        }
        if let Some((g, slot)) = open {
            cur = self.close_group(&mut passes, &t, comp, g, slot, cur, now, inputs);
        }
        let mixed = self.effect_chain(&mut passes, &t.fx, t.comp[cur].clone(), &comp.effects, now, inputs);
        passes.push(Self::blend_pass(&t.output, &mixed, &mixed, MODE_MASTER, comp.master_value(inputs)));
        if let Some(shared) = &self.shared {
            passes.push(Self::blend_pass(shared, &t.output, &t.output, MODE_MIX, 0.0));
        }
        // A clip that is on screen was copied into the preview, with its effects, as it was
        // drawn. Any other previewed clip is drawn here, through its own effects.
        if !self.previewed {
            match preview.and_then(|id| comp.find_clip(id)).and_then(|(l, c)| comp.clip(l, c)) {
                Some(clip) => {
                    let time = clip_time(comp, clip, now, inputs.beat);
                    if clip_has_fx(clip) {
                        self.draw_clip(&mut passes, &t, clip, time, &t.clip_a, now, inputs, None, 1);
                        passes.push(Self::blend_pass(&t.preview, &t.clip_a, &t.clip_a, MODE_MIX, 0.0));
                    } else {
                        self.clip_passes(&mut passes, &t.fx, clip, time, inputs, &t.preview, 1);
                    }
                }
                None => passes.push(Pass::clear(&t.preview, wgpu::Color::BLACK)),
            }
        }
        for clip in comp.all_clips().filter(|_| refresh_thumbs) {
            if let Some(thumb) = self.thumbs.get(&clip.id).cloned() {
                let time = clip_time(comp, clip, now, inputs.beat);
                self.clip_passes(&mut passes, &t.fx, clip, time, inputs, &thumb, 2);
            }
        }
        // Effects and generators that didn't run this frame lose their feedback buffers.
        let seen = std::mem::take(&mut self.fx_seen);
        self.fx_state.retain(|uid, _| seen.contains(uid));

        self.submit(passes);
    }

    /// Render passes the last [`Engine::render`] encoded.
    pub fn last_pass_count(&self) -> usize {
        self.last_passes
    }

    fn submit(&mut self, passes: Vec<Pass>) {
        self.last_passes = passes.len();
        let needed = passes.len() as u64;
        if needed > self.uniform_slots {
            self.uniform_slots = needed.next_power_of_two();
            self.uniform_buf = Self::create_uniform_buffer(&self.device, self.uniform_slots * UNIFORM_STRIDE);
        }
        let mut data = vec![0u8; (needed * UNIFORM_STRIDE) as usize];
        for (i, p) in passes.iter().enumerate() {
            let off = i * UNIFORM_STRIDE as usize;
            data[off..off + size_of::<Uniforms>()].copy_from_slice(bytemuck::bytes_of(&p.uniforms));
        }
        if !data.is_empty() {
            self.queue.write_buffer(&self.uniform_buf, 0, &data);
        }

        // Effect uniform blocks, each at a 256-byte aligned offset.
        let mut fx_data = Vec::new();
        let mut fx_offsets = Vec::new();
        for p in &passes {
            if let Draw::Isf { blocks, .. } = &p.draw {
                for (_, bytes) in blocks {
                    fx_offsets.push(fx_data.len() as u64);
                    fx_data.extend_from_slice(bytes);
                    fx_data.resize(fx_data.len().next_multiple_of(UNIFORM_STRIDE as usize), 0);
                }
            }
        }
        if fx_data.len() as u64 > self.fx_uniform_buf.size() {
            let size = (fx_data.len() as u64).next_power_of_two();
            self.fx_uniform_buf = Self::create_uniform_buffer(&self.device, size);
        }
        if !fx_data.is_empty() {
            self.queue.write_buffer(&self.fx_uniform_buf, 0, &fx_data);
        }

        let mut encoder =
            self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("openvj frame") });
        let mut fx_block = 0;
        for (i, p) in passes.iter().enumerate() {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &p.target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(p.clear), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            match &p.draw {
                Draw::Clear => {}
                Draw::Builtin(kind) => {
                    let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &self.layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                    buffer: &self.uniform_buf,
                                    offset: 0,
                                    size: wgpu::BufferSize::new(size_of::<Uniforms>() as u64),
                                }),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&self.sampler),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::TextureView(p.a.as_ref().unwrap_or(&self.dummy)),
                            },
                            wgpu::BindGroupEntry {
                                binding: 3,
                                resource: wgpu::BindingResource::TextureView(p.b.as_ref().unwrap_or(&self.dummy)),
                            },
                        ],
                    });
                    rp.set_pipeline(&self.pipelines[&(*kind, p.format)]);
                    rp.set_bind_group(0, &bind_group, &[(i as u64 * UNIFORM_STRIDE) as u32]);
                    rp.draw(0..3, 0..1);
                }
                Draw::Isf { fx, blocks, textures } => {
                    let mut entries = Vec::new();
                    for (binding, bytes) in blocks {
                        entries.push(wgpu::BindGroupEntry {
                            binding: *binding,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &self.fx_uniform_buf,
                                offset: fx_offsets[fx_block],
                                size: wgpu::BufferSize::new(bytes.len() as u64),
                            }),
                        });
                        fx_block += 1;
                    }
                    for (binding, _) in &fx.textures {
                        let view = textures.iter().find(|(b, _)| b == binding).map_or(&self.dummy, |(_, v)| v);
                        entries.push(wgpu::BindGroupEntry {
                            binding: *binding,
                            resource: wgpu::BindingResource::TextureView(view),
                        });
                    }
                    if let Some(binding) = fx.sampler {
                        entries.push(wgpu::BindGroupEntry {
                            binding,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        });
                    }
                    let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &fx.layout,
                        entries: &entries,
                    });
                    rp.set_pipeline(&fx.pipeline);
                    rp.set_bind_group(0, &bind_group, &[]);
                    rp.draw(0..3, 0..1);
                }
            }
        }
        self.queue.submit([encoder.finish()]);
    }

    /// Reads a display target back to the CPU as tightly packed RGBA8 (BGRA8 for
    /// [`Engine::shared_output`]). Blocks; for tests and snapshots.
    pub fn read_target(&self, target: &Target) -> Vec<u8> {
        assert!(matches!(target.format, LDR | SHARE), "only display targets can be read back");
        let (w, h) = (target.width, target.height);
        let padded = (w * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            target.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: None },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("map readback buffer"));
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll device");
        let mapped = slice.get_mapped_range().expect("read mapped buffer");
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for row in mapped.chunks(padded as usize) {
            out.extend_from_slice(&row[..(w * 4) as usize]);
        }
        out
    }
}

/// Local time of a clip: its playing position if it's live on its layer, otherwise
/// free-running from the start of the composition clock.
/// Whether a clip's Transform or effects change what it draws.
fn clip_has_fx(clip: &Clip) -> bool {
    !clip.transform.is_noop() || clip.effects.iter().any(|fx| fx.enabled)
}

fn clip_time(comp: &Composition, clip: &Clip, now: f64, beat: f64) -> f64 {
    match comp.layers.iter().find_map(|l| l.active.filter(|p| p.clip_id == clip.id)) {
        Some(p) => p.time(clip, now, beat).secs,
        None => clip.time_at(now, beat).secs,
    }
}

/// UV scale that makes a `sw`×`sh` source cover a `tw`×`th` target, cropping the excess.
fn cover_scale(sw: u32, sh: u32, tw: u32, th: u32) -> [f32; 2] {
    let src = sw as f32 / sh as f32;
    let dst = tw as f32 / th as f32;
    if src > dst { [dst / src, 1.0] } else { [1.0, src / dst] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openvj_core::{BlendMode, GeneratorKind, ModSource, Modulation};

    fn inputs(now: f64) -> ModInputs {
        ModInputs { beat: openvj_core::beats(now, 120.0), bands: [0.0; 4] }
    }

    fn engine(w: u32, h: u32) -> Option<Engine> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: adapter.features() & wgpu::Features::TEXTURE_COMPRESSION_BC,
            required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .ok()?;
        Some(Engine::new(device, queue, w, h))
    }

    fn mean(px: &[u8]) -> [f32; 3] {
        let n = (px.len() / 4) as f32;
        let mut s = [0f32; 3];
        for p in px.chunks(4) {
            for i in 0..3 {
                s[i] += p[i] as f32;
            }
        }
        s.map(|v| v / n)
    }

    #[test]
    fn cover_scale_crops_the_long_side() {
        assert_eq!(cover_scale(200, 100, 100, 100), [0.5, 1.0]);
        assert_eq!(cover_scale(100, 200, 100, 100), [1.0, 0.5]);
    }

    /// Mean colour of the pixels of `px` (w wide) in columns `x0..x1`.
    fn mean_cols(px: &[u8], w: usize, x0: usize, x1: usize) -> [f32; 3] {
        let cols: Vec<u8> = px.chunks(w * 4).flat_map(|row| row[x0 * 4..x1 * 4].to_vec()).collect();
        mean(&cols)
    }

    #[test]
    fn maps_slices_through_warps_and_masks() {
        use openvj_core::{Contour, Mask, ScreenLayer, rect_quad};
        let Some(mut e) = engine(64, 36) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let mut comp = Composition::new(64, 36, 1, 1);
        // Red on the left half of the composition, blue on the right.
        let img = comp.set_clip(0, 0, ClipSource::Image { path: "rb.png".into() }).unwrap();
        let mut px = Vec::new();
        for _ in 0..36 {
            for x in 0..64 {
                px.extend_from_slice(if x < 32 { &[255, 0, 0, 255] } else { &[0, 0, 255, 255] });
            }
        }
        e.set_media_frame(img, 64, 36, &px);
        comp.trigger(0, 0, 0.0);
        e.render(&comp, 0.1, &inputs(0.1), None);

        // Passthrough: the screen is the composition.
        let mut setup = OutputSetup::passthrough(64, 36);
        e.map_output(&setup, false);
        let out = e.read_target(e.screen(0).unwrap());
        assert!(mean_cols(&out, 64, 0, 28)[0] > 250.0 && mean_cols(&out, 64, 36, 64)[2] > 250.0);

        // Input: the right half only, stretched over the screen.
        let ScreenLayer::Slice(slice) = &mut setup.screens[0].layers[0] else { unreachable!() };
        slice.input = rect_quad(32.0, 0.0, 64.0, 36.0);
        e.map_output(&setup, false);
        let m = mean(&e.read_target(e.screen(0).unwrap()));
        assert!(m[2] > 250.0 && m[0] < 8.0, "right half only: {m:?}");

        // Corner pin onto the left half of the screen: the right half goes black.
        let ScreenLayer::Slice(slice) = &mut setup.screens[0].layers[0] else { unreachable!() };
        slice.warp.dst = rect_quad(0.0, 0.0, 32.0, 36.0);
        e.map_output(&setup, false);
        let out = e.read_target(e.screen(0).unwrap());
        assert!(mean_cols(&out, 64, 0, 30)[2] > 250.0, "pinned half: {:?}", mean_cols(&out, 64, 0, 30));
        assert_eq!(mean_cols(&out, 64, 34, 64), [0.0; 3]);

        // An inverted mask over the top half shows only that.
        setup.screens[0].layers.push(ScreenLayer::Mask(Mask {
            id: 9,
            name: "top".into(),
            enabled: true,
            invert: true,
            contours: vec![Contour::polygon(rect_quad(0.0, 0.0, 64.0, 18.0).to_vec())],
        }));
        e.map_output(&setup, false);
        let out = e.read_target(e.screen(0).unwrap());
        let (top, bottom) = out.split_at(64 * 4 * 18);
        assert!(mean_cols(top, 64, 0, 30)[2] > 250.0);
        assert_eq!(mean(bottom), [0.0; 3]);

        // The test card replaces the composition.
        e.map_output(&setup, true);
        let out = e.read_target(e.screen(0).unwrap());
        let (top, _) = out.split_at(64 * 4 * 18);
        let m = mean_cols(top, 64, 0, 30);
        assert!(m[0] > 1.0 && m[1] > 1.0, "test card: {m:?}");
    }

    #[test]
    fn renders_layers_and_blends() {
        let Some(mut e) = engine(64, 36) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let mut comp = Composition::new(64, 36, 2, 1);
        // Empty composition renders black.
        e.render(&comp, 0.0, &inputs(0.0), None);
        assert_eq!(mean(&e.read_target(e.output())), [0.0; 3]);

        // A solid red image on layer 0.
        let red = comp.set_clip(0, 0, ClipSource::Image { path: "red.png".into() }).unwrap();
        e.set_media_frame(red, 2, 2, &[255, 0, 0, 255].repeat(4));
        comp.trigger(0, 0, 0.0);
        e.render(&comp, 0.1, &inputs(0.1), None);
        let m = mean(&e.read_target(e.output()));
        assert!(m[0] > 250.0 && m[1] < 1.0, "red: {m:?}");

        // Solid blue on layer 1, added on top at half opacity.
        let blue = comp.set_clip(1, 0, ClipSource::Image { path: "blue.png".into() }).unwrap();
        e.set_media_frame(blue, 2, 2, &[0, 0, 255, 255].repeat(4));
        comp.layers[1].blend = BlendMode::Add;
        comp.layers[1].opacity = 0.5;
        comp.trigger(1, 0, 0.0);
        e.render(&comp, 0.1, &inputs(0.1), None);
        let m = mean(&e.read_target(e.output()));
        assert!(m[0] > 250.0 && (m[2] - 127.5).abs() < 2.0, "red + half blue: {m:?}");

        // Master fader, and the preview shows a clip on its own.
        comp.master = 0.0;
        e.render(&comp, 0.1, &inputs(0.1), Some(blue));
        assert_eq!(mean(&e.read_target(e.output())), [0.0; 3]);
        let p = mean(&e.read_target(e.preview()));
        assert!(p[2] > 250.0 && p[0] < 1.0, "preview shows blue: {p:?}");
    }

    /// A BC1 block of one RGB565 colour.
    fn bc1(c: u16) -> [u8; 8] {
        let [a, b] = c.to_le_bytes();
        [a, b, a, b, 0, 0, 0, 0]
    }

    /// An RGTC (BC4) block of one value.
    fn rgtc(v: u8) -> [u8; 8] {
        [v, v, 0, 0, 0, 0, 0, 0]
    }

    fn render_clip(e: &mut Engine, frame: &BcFrame) -> Vec<u8> {
        let mut comp = Composition::new(e.size().0, e.size().1, 1, 1);
        let id = comp.set_clip(0, 0, ClipSource::Video { path: "clip.mov".into() }).unwrap();
        e.set_media_bc(id, frame);
        comp.trigger(0, 0, 0.0);
        e.render(&comp, 0.1, &inputs(0.1), None);
        e.read_target(e.output())
    }

    #[test]
    fn samples_block_compressed_frames() {
        let Some(mut e) = engine(24, 16) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        if !e.supports_bc() {
            eprintln!("skipping: GPU without BC textures");
            return;
        }
        let max = |px: &[u8], c: usize| px.chunks(4).map(|p| p[c]).max().unwrap();
        let min = |px: &[u8], c: usize| px.chunks(4).map(|p| p[c]).min().unwrap();

        // BC1: a 12×8 picture coded as 16×8. The padding column is green and must not show.
        let red = bc1(0xF800);
        let green = bc1(0x07E0);
        let data: Vec<u8> = [red, red, red, green, red, red, red, green].concat();
        let frame = BcFrame {
            layout: BcLayout::Bc1Opaque,
            width: 12,
            height: 8,
            coded_width: 16,
            coded_height: 8,
            data: &data,
            chroma: &[],
            chroma_height: 0,
        };
        let px = render_clip(&mut e, &frame);
        assert!(min(&px, 0) > 250 && max(&px, 1) < 3, "BC1 red without padding: {:?}", mean(&px));

        // BC3 with premultiplied alpha: half-transparent white (128, a = 128) is straight white.
        let mut block = vec![128, 128, 0, 0, 0, 0, 0, 0];
        block.extend(bc1(0x8410)); // RGB565 grey ≈ 132
        let data = block.repeat(8);
        let frame = BcFrame { layout: BcLayout::Bc3Premultiplied, data: &data, ..frame };
        // The composition is opaque black underneath, so straight white at alpha ½ shows as grey.
        let m = mean(&render_clip(&mut e, &frame));
        assert!((m[0] - 128.0).abs() < 6.0, "BC3 premultiplied: {m:?}");

        // YCoCg: RGB (200, 100, 50) is Y 112.5, Co +75, Cg −12.5.
        let y = rgtc(113).repeat(8);
        let mut cocg = rgtc(203).to_vec();
        cocg.extend(rgtc(116));
        let cocg = cocg.repeat(2); // 8×4 chroma for a 16×8 frame
        let frame = BcFrame { layout: BcLayout::YCoCg, data: &y, chroma: &cocg, chroma_height: 4, ..frame };
        let m = mean(&render_clip(&mut e, &frame));
        for (got, want) in m.iter().zip([200.0, 100.0, 50.0]) {
            assert!((got - want).abs() < 4.0, "YCoCg: {m:?}");
        }

        // YCoCg + alpha: luma and alpha share a BC5 texture; alpha 0 leaves the black underneath.
        let ya: Vec<u8> = [rgtc(113), rgtc(0)].concat().repeat(8);
        let frame = BcFrame { layout: BcLayout::YCoCgAlpha, data: &ya, ..frame };
        let m = mean(&render_clip(&mut e, &frame));
        assert!(m.iter().all(|v| *v < 2.0), "YCoCg alpha 0: {m:?}");

        // BC3 with straight alpha (HAP Alpha): grey 132 at alpha ½ over black is ≈ 66.
        let frame = BcFrame { layout: BcLayout::Bc3, data: &data, chroma: &[], chroma_height: 0, ..frame };
        let m = mean(&render_clip(&mut e, &frame));
        assert!((m[0] - 66.0).abs() < 4.0, "BC3 straight: {m:?}");

        // Scaled YCoCg (HAP Q): Co/Cg in the 565 colour, the scale in blue, luma in alpha.
        let (r5, g6, b5, luma) = (22u16, 31u16, 1u16, 113u8);
        let mut block = vec![luma, luma, 0, 0, 0, 0, 0, 0];
        block.extend(bc1(r5 << 11 | g6 << 5 | b5));
        let ycocg = block.repeat(8);
        let scale = b5 as f32 / 31.0 * 255.0 / 8.0 + 1.0;
        let co = (r5 as f32 / 31.0 * 255.0 - 128.0) / scale;
        let cg = (g6 as f32 / 63.0 * 255.0 - 128.0) / scale;
        let y = luma as f32;
        let want = [y + co - cg, y + cg, y - co - cg];
        let frame = BcFrame { layout: BcLayout::ScaledYCoCg, data: &ycocg, ..frame };
        let m = mean(&render_clip(&mut e, &frame));
        for (got, want) in m.iter().zip(want) {
            assert!((got - want).abs() < 3.0, "scaled YCoCg: {m:?}, want {want:?}");
        }

        // Scaled YCoCg + full-size BC4 alpha (HAP Q Alpha): alpha ½ halves the colour over black.
        let alpha = rgtc(128).repeat(8);
        let frame = BcFrame { layout: BcLayout::ScaledYCoCgAlpha, chroma: &alpha, chroma_height: 8, ..frame };
        let m = mean(&render_clip(&mut e, &frame));
        for (got, want) in m.iter().zip(want) {
            assert!((got - want * 128.0 / 255.0).abs() < 3.0, "scaled YCoCg alpha: {m:?}");
        }

        // BC7 (HAP R): a mode 6 block of one colour, RGBA (200, 100, 50, 254).
        let mut bits = 0u128;
        let mut at = 0;
        let mut push = |v: u128, n: u32| {
            bits |= v << at;
            at += n;
        };
        push(1 << 6, 7); // mode 6
        for c in [100, 100, 50, 50, 25, 25, 127, 127] {
            push(c, 7); // endpoints 0 and 1 of R, G, B, A, 7 bits each
        }
        // P bits and indices stay 0: every pixel is endpoint 0, (c << 1) | 0.
        let data = bits.to_le_bytes().repeat(8);
        let frame = BcFrame { layout: BcLayout::Bc7, data: &data, chroma: &[], chroma_height: 0, ..frame };
        let m = mean(&render_clip(&mut e, &frame));
        for (got, want) in m.iter().zip([200.0, 100.0, 50.0]) {
            assert!((got - want).abs() < 3.0, "BC7: {m:?}");
        }
    }

    #[test]
    fn groups_mix_their_layers_then_blend_as_one() {
        let Some(mut e) = engine(32, 32) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let mut comp = Composition::new(32, 32, 3, 1);
        for (layer, rgba) in [(0, [255, 0, 0, 255]), (1, [0, 255, 0, 255]), (2, [0, 0, 255, 255])] {
            let id = comp.set_clip(layer, 0, ClipSource::Image { path: "c.png".into() }).unwrap();
            e.set_media_frame(id, 2, 2, &rgba.repeat(4));
        }
        comp.trigger_column(0, 0.0);
        // Green and blue added together inside a group, then the group at half opacity.
        comp.layers[2].blend = BlendMode::Add;
        let g = comp.group_layers(&[1, 2]).unwrap();
        comp.groups[g].opacity = 0.5;
        e.render(&comp, 0.1, &inputs(0.1), None);
        let m = mean(&e.read_target(e.output()));
        assert!((m[0] - 127.5).abs() < 2.0 && (m[1] - 127.5).abs() < 2.0 && (m[2] - 127.5).abs() < 2.0, "{m:?}");

        // Group effects apply to the group's mix only.
        comp.groups[g].opacity = 1.0;
        comp.groups[g].effects.push(Effect::new("invert"));
        e.render(&comp, 0.1, &inputs(0.1), None);
        let m = mean(&e.read_target(e.output()));
        assert!(m[0] > 250.0 && m[1] < 2.0 && m[2] < 2.0, "inverted cyan is red over red: {m:?}");

        // A bypassed group hides its layers.
        comp.groups[g].bypass = true;
        e.render(&comp, 0.1, &inputs(0.1), None);
        let m = mean(&e.read_target(e.output()));
        assert!(m[0] > 250.0 && m[1] < 2.0 && m[2] < 2.0, "{m:?}");
    }

    #[test]
    fn crossfader_fades_layers_on_its_sides() {
        let Some(mut e) = engine(32, 32) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let mut comp = Composition::new(32, 32, 2, 1);
        for (layer, rgba) in [(0, [255, 0, 0, 255]), (1, [0, 0, 255, 255])] {
            let id = comp.set_clip(layer, 0, ClipSource::Image { path: "c.png".into() }).unwrap();
            e.set_media_frame(id, 2, 2, &rgba.repeat(4));
            comp.layers[layer].blend = BlendMode::Add;
        }
        comp.layers[0].crossfade = openvj_core::CrossfadeSide::A;
        comp.layers[1].crossfade = openvj_core::CrossfadeSide::B;
        comp.trigger_column(0, 0.0);
        comp.crossfader.position = 0.0;
        e.render(&comp, 0.1, &inputs(0.1), None);
        let m = mean(&e.read_target(e.output()));
        assert!(m[0] > 250.0 && m[2] < 2.0, "all A: {m:?}");
        comp.crossfader.position = 0.75;
        e.render(&comp, 0.1, &inputs(0.1), None);
        let m = mean(&e.read_target(e.output()));
        assert!((m[0] - 63.75).abs() < 2.0 && (m[2] - 191.25).abs() < 2.0, "mostly B: {m:?}");
    }

    #[test]
    fn crossfade_mixes_clips() {
        let Some(mut e) = engine(32, 32) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let mut comp = Composition::new(32, 32, 1, 2);
        let white = comp.set_clip(0, 0, ClipSource::Image { path: "w.png".into() }).unwrap();
        let black = comp.set_clip(0, 1, ClipSource::Image { path: "b.png".into() }).unwrap();
        e.set_media_frame(white, 1, 1, &[255; 4]);
        e.set_media_frame(black, 1, 1, &[0, 0, 0, 255]);
        comp.layers[0].transition_secs = 1.0;
        comp.trigger(0, 0, 0.0);
        comp.tick(2.0, 4.0);
        comp.trigger(0, 1, 2.0);
        e.render(&comp, 2.25, &inputs(2.25), None);
        let m = mean(&e.read_target(e.output()));
        assert!((m[0] - 191.0).abs() < 3.0, "75% white: {m:?}");
    }

    #[test]
    fn layer_and_composition_effects_chain() {
        let Some(mut e) = engine(32, 32) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let mut comp = Composition::new(32, 32, 1, 1);
        let red = comp.set_clip(0, 0, ClipSource::Image { path: "r.png".into() }).unwrap();
        e.set_media_frame(red, 1, 1, &[255, 0, 0, 255]);
        comp.trigger(0, 0, 0.0);

        // Layer invert turns red into cyan.
        comp.layers[0].effects.push(Effect::new("invert"));
        e.render(&comp, 0.1, &inputs(0.1), None);
        let m = mean(&e.read_target(e.output()));
        assert!(m[0] < 1.0 && m[1] > 250.0 && m[2] > 250.0, "cyan: {m:?}");

        // A second invert in the layer chain cancels it out.
        comp.layers[0].effects.push(Effect::new("invert"));
        e.render(&comp, 0.1, &inputs(0.1), None);
        let m = mean(&e.read_target(e.output()));
        assert!(m[0] > 250.0 && m[1] < 1.0, "red again: {m:?}");

        // Disabled effects are skipped; a composition invert at half mix gives grey-ish.
        comp.layers[0].effects[1].enabled = false;
        let mut fx = Effect::new("invert");
        fx.mix.value = 0.5;
        comp.effects.push(fx);
        e.render(&comp, 0.1, &inputs(0.1), None);
        let m = mean(&e.read_target(e.output()));
        assert!(m.iter().all(|c| (c - 127.5).abs() < 2.0), "half-inverted cyan: {m:?}");
    }

    #[test]
    fn clip_effects_and_transform() {
        let Some(mut e) = engine(32, 32) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let px = |e: &Engine, t: &Target, x: usize, y: usize| {
            let p = e.read_target(t);
            let i = (y * 32 + x) * 4;
            [p[i], p[i + 1], p[i + 2]]
        };
        let mut comp = Composition::new(32, 32, 1, 2);
        let red = comp.set_clip(0, 0, ClipSource::Image { path: "r.png".into() }).unwrap();
        e.set_media_frame(red, 1, 1, &[255, 0, 0, 255]);
        comp.trigger(0, 0, 0.0);
        e.render(&comp, 0.1, &inputs(0.1), Some(red));
        let plain = e.last_pass_count();

        // A Transform at its defaults costs nothing.
        comp.layers[0].clips[0].as_mut().unwrap().transform.param_mut("scale").unwrap().value = 0.1;
        comp.layers[0].clips[0].as_mut().unwrap().transform = openvj_core::default_transform();
        e.render(&comp, 0.1, &inputs(0.1), Some(red));
        assert_eq!(e.last_pass_count(), plain);

        // A clip effect applies to that clip only, and the preview shows it.
        comp.layers[0].clips[0].as_mut().unwrap().effects.push(Effect::new("invert"));
        e.render(&comp, 0.1, &inputs(0.1), Some(red));
        assert_eq!(px(&e, e.output(), 16, 16), [0, 255, 255]);
        assert_eq!(px(&e, e.preview(), 16, 16), [0, 255, 255]);
        let other = comp.set_clip(0, 1, ClipSource::Image { path: "g.png".into() }).unwrap();
        e.set_media_frame(other, 1, 1, &[0, 255, 0, 255]);
        comp.trigger(0, 1, 0.2);
        e.render(&comp, 0.3, &inputs(0.3), Some(red));
        assert_eq!(px(&e, e.output(), 16, 16), [0, 255, 0], "the other clip plays unprocessed");
        assert_eq!(px(&e, e.preview(), 16, 16), [0, 255, 255], "a clip off screen previews with its effects");

        // Position X moves right by pixels, Y up; the uncovered part is empty.
        comp.trigger(0, 0, 0.4);
        let clip = comp.layers[0].clips[0].as_mut().unwrap();
        clip.effects.clear();
        let tf = clip.transform.ty().unwrap();
        let x = tf.def.inputs.iter().position(|i| i.name == "position_x").unwrap();
        clip.transform.param_mut("position_x").unwrap().value = tf.def.inputs[x].norm(0, 16.0);
        e.render(&comp, 0.5, &inputs(0.5), None);
        assert_eq!(px(&e, e.output(), 4, 16), [0, 0, 0]);
        assert_eq!(px(&e, e.output(), 28, 16), [255, 0, 0]);
        let clip = comp.layers[0].clips[0].as_mut().unwrap();
        clip.transform = openvj_core::default_transform();
        clip.transform.param_mut("position_y").unwrap().value = tf.def.inputs[x + 1].norm(0, 16.0);
        e.render(&comp, 0.6, &inputs(0.6), None);
        assert_eq!(px(&e, e.output(), 16, 4), [255, 0, 0], "moved up");
        assert_eq!(px(&e, e.output(), 16, 28), [0, 0, 0]);

        // A layer Transform at half scale leaves a border; a quarter turn in X hides the card.
        comp.layers[0].clips[0].as_mut().unwrap().transform = openvj_core::default_transform();
        comp.layers[0].transform.param_mut("scale").unwrap().value = 0.05;
        e.render(&comp, 0.7, &inputs(0.7), None);
        assert_eq!(px(&e, e.output(), 16, 16), [255, 0, 0]);
        assert_eq!(px(&e, e.output(), 2, 2), [0, 0, 0]);
        comp.layers[0].transform = openvj_core::default_transform();
        comp.layers[0].transform.param_mut("rotation_x").unwrap().value = 0.75;
        e.render(&comp, 0.8, &inputs(0.8), None);
        assert_eq!(px(&e, e.output(), 16, 16), [0, 0, 0], "edge-on");
        comp.layers[0].transform.param_mut("rotation_x").unwrap().value = 0.6;
        e.render(&comp, 0.9, &inputs(0.9), None);
        assert_eq!(px(&e, e.output(), 16, 16), [255, 0, 0], "tilted, still covering the centre");
    }

    #[test]
    fn audio_band_modulates_layer_opacity() {
        let Some(mut e) = engine(16, 16) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let mut comp = Composition::new(16, 16, 1, 1);
        let white = comp.set_clip(0, 0, ClipSource::Image { path: "w.png".into() }).unwrap();
        e.set_media_frame(white, 1, 1, &[255; 4]);
        comp.trigger(0, 0, 0.0);
        comp.layers[0].opacity = 0.0;
        comp.layers[0].opacity_mod = Modulation { source: ModSource::Bass, amount: 1.0, beats: 1.0 };
        let mut i = inputs(0.1);
        e.render(&comp, 0.1, &i, None);
        assert_eq!(mean(&e.read_target(e.output())), [0.0; 3]);
        i.bands[0] = 0.5;
        e.render(&comp, 0.1, &i, None);
        let m = mean(&e.read_target(e.output()));
        assert!((m[0] - 127.5).abs() < 2.0, "half bass, half white: {m:?}");
    }

    #[test]
    fn live_thumbnails_refresh_at_their_own_rate() {
        let Some(mut e) = engine(64, 36) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let mut comp = Composition::new(64, 36, 1, 8);
        for col in 0..8 {
            comp.set_clip(0, col, ClipSource::Generator { kind: GeneratorKind::ALL[0] }).unwrap();
        }
        e.render(&comp, 1.0, &inputs(1.0), None);
        let with_thumbs = e.last_pass_count();
        // A frame later: no thumbnail passes.
        e.render(&comp, 1.016, &inputs(1.016), None);
        assert_eq!(e.last_pass_count(), with_thumbs - 8);
        let later = 1.001 + THUMB_INTERVAL;
        e.render(&comp, later, &inputs(later), None);
        assert_eq!(e.last_pass_count(), with_thumbs);
        // An idle composition is a handful of passes, whatever the grid holds.
        assert!(e.last_pass_count() - 8 <= 4, "{} passes", e.last_pass_count());
    }

    #[test]
    fn shared_output_is_a_bgra_copy_of_the_output() {
        let Some(mut e) = engine(64, 36) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let mut comp = Composition::new(64, 36, 1, 1);
        comp.set_clip(0, 0, ClipSource::Generator { kind: GeneratorKind::ALL[0] }).unwrap();
        comp.trigger(0, 0, 0.0);
        e.render(&comp, 0.05, &inputs(0.05), None);
        assert!(e.shared_output().is_none(), "off unless asked for");

        e.set_shared_output(true);
        e.render(&comp, 0.05, &inputs(0.05), None);
        let rgba = e.read_target(e.output());
        let bgra = e.read_target(e.shared_output().unwrap());
        let swapped: Vec<u8> = bgra.chunks(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
        let worst = rgba.iter().zip(&swapped).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
        assert!(worst <= 1, "BGRA copy differs by {worst}");
        assert!(mean(&rgba).iter().any(|c| *c > 1.0), "the generator drew something");

        e.resize(32, 18);
        let shared = e.shared_output().unwrap();
        assert_eq!((shared.texture.width(), shared.texture.height()), (32, 18));
        e.set_shared_output(false);
        assert!(e.shared_output().is_none());
    }

    #[test]
    fn every_generator_draws_something() {
        let Some(mut e) = engine(64, 36) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let mut comp = Composition::new(64, 36, 1, GeneratorKind::ALL.len());
        let ids: Vec<_> = GeneratorKind::ALL
            .iter()
            .enumerate()
            .map(|(col, kind)| comp.set_clip(0, col, ClipSource::Generator { kind: *kind }).unwrap())
            .collect();
        // Early in the beat so the strobe and bars are lit.
        e.render(&comp, 0.05, &inputs(0.05), None);
        for (kind, id) in GeneratorKind::ALL.iter().zip(ids) {
            let thumb = e.thumb(id).clone();
            let m = mean(&e.read_target(&thumb));
            assert!(m.iter().sum::<f32>() > 3.0, "{} is black: {m:?}", kind.name());
        }
    }

    /// A 32×32 composition with a half-white, half-black image on one layer.
    fn split_comp(e: &mut Engine) -> (Composition, ClipId) {
        let mut comp = Composition::new(32, 32, 1, 1);
        let img = comp.set_clip(0, 0, ClipSource::Image { path: "split.png".into() }).unwrap();
        e.set_media_frame(img, 2, 1, &[255, 255, 255, 255, 0, 0, 0, 255]);
        comp.trigger(0, 0, 0.0);
        (comp, img)
    }

    /// Tests that swap the global effect library take this so they don't race.
    static LIBRARY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn changed_effect_files_recompile() {
        use openvj_core::{EffectLibrary, EffectType, isf::EffectDef, set_library};
        let _lock = LIBRARY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut e) = engine(32, 32) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let install = |red: f32| {
            let src = format!(
                r#"/*{{ "INPUTS": [{{"NAME": "inputImage", "TYPE": "image"}}] }}*/
                void main() {{ gl_FragColor = vec4({red:.1}, 0.0, 0.0, 1.0); }}"#
            );
            let def = EffectDef::parse(&src, "hot_reload_test").unwrap();
            check_effect(&def).unwrap();
            set_library(EffectLibrary::with(vec![std::sync::Arc::new(EffectType::new(def, None))], Vec::new()));
        };
        let (mut comp, _) = split_comp(&mut e);
        comp.layers[0].effects = vec![Effect::new("hot_reload_test")];
        let mut red = || {
            e.render(&comp, 0.1, &inputs(0.1), None);
            mean(&e.read_target(e.output()))[0]
        };
        install(1.0);
        assert!(red() > 250.0);
        install(0.0);
        assert!(red() < 1.0, "the new version runs");
        // A file that disappears passes video through (the split image averages to half).
        set_library(EffectLibrary::builtin());
        assert!((red() - 127.5).abs() < 3.0);
    }

    #[test]
    fn isf_generators_play_as_clips() {
        use openvj_core::{EffectLibrary, EffectType, isf::EffectDef, set_library};
        let _lock = LIBRARY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut e) = engine(32, 32) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        // Red follows the `level` input, green follows time, blue the bass band (Ghost Arcade name).
        let src = r#"/*{ "INPUTS": [{"NAME": "level", "TYPE": "float", "DEFAULT": 1.0}] }*/
            void main() { gl_FragColor = vec4(level, fract(TIME), audioBass, 1.0); }"#;
        let def = EffectDef::parse(src, "isf_gen_test").unwrap();
        check_effect(&def).unwrap();
        let ty = std::sync::Arc::new(EffectType::new(def, None));
        assert!(ty.is_generator());
        set_library(EffectLibrary::with(Vec::new(), Vec::new()).with_generators(vec![ty]));

        let mut comp = Composition::new(32, 32, 1, 1);
        let id = comp.set_clip(0, 0, ClipSource::isf("isf_gen_test")).unwrap();
        comp.trigger(0, 0, 0.0);
        e.thumb(id);
        let mut i = inputs(0.25);
        i.bands[0] = 1.0;
        e.render(&comp, 0.25, &i, Some(id));
        let m = mean(&e.read_target(e.output()));
        assert!(m[0] > 250.0 && (m[1] - 63.75).abs() < 3.0 && m[2] > 250.0, "red, quarter green, bass blue: {m:?}");
        let p = mean(&e.read_target(e.preview()));
        assert!(p[0] > 250.0 && p[2] > 250.0, "preview: {p:?}");
        let thumb = e.thumb(id).clone();
        let t = mean(&e.read_target(&thumb));
        assert!(t[0] > 250.0, "thumbnail: {t:?}");

        // Its parameters work like an effect's.
        let ClipSource::Isf { generator } = &mut comp.clip_mut(0, 0).unwrap().source else { panic!() };
        generator.param_mut("level").unwrap().value = 0.0;
        e.render(&comp, 0.5, &inputs(0.5), None);
        let m = mean(&e.read_target(e.output()));
        assert!(m[0] < 1.0 && (m[1] - 127.5).abs() < 3.0, "time moved on, level 0: {m:?}");

        // A missing generator draws nothing.
        set_library(EffectLibrary::builtin());
        e.render(&comp, 0.5, &inputs(0.5), None);
        assert_eq!(mean(&e.read_target(e.output())), [0.0; 3]);
    }

    /// Slow (every bundled shader is compiled for the GPU): run after re-importing the pack with
    /// `cargo test -p openvj-render bundled_shaders_render -- --ignored`.
    #[test]
    #[ignore]
    fn bundled_shaders_render() {
        use openvj_core::{effect_folders::Scanner, set_library};
        let _lock = LIBRARY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut e = engine(32, 32).expect("a GPU adapter");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shaders");
        let r = Scanner::new(vec![dir], check_effect).scan().unwrap();
        let lib = r.into_library();
        let gens: Vec<String> = lib.generators().iter().map(|t| t.id().to_owned()).collect();
        let fxs: Vec<String> = lib.all().iter().filter(|t| t.path.is_some()).map(|t| t.id().to_owned()).collect();
        set_library(lib);
        let mut failed = Vec::new();
        let (mut fx_comp, _) = split_comp(&mut e);
        for (id, is_gen) in gens.iter().map(|g| (g, true)).chain(fxs.iter().map(|f| (f, false))) {
            let comp = if is_gen {
                let mut c = Composition::new(32, 32, 1, 1);
                c.set_clip(0, 0, ClipSource::isf(id));
                c.trigger(0, 0, 0.0);
                c
            } else {
                fx_comp.layers[0].effects = vec![Effect::new(id)];
                fx_comp.clone()
            };
            let scope = e.device.push_error_scope(wgpu::ErrorFilter::Validation);
            for frame in 0..2 {
                let now = 1.0 + frame as f64 * 0.5;
                e.render(&comp, now, &inputs(now), None);
            }
            let gpu_error = pollster::block_on(scope.pop());
            let m = mean(&e.read_target(e.output()));
            if let Some(err) = e.effect_error(id).map(str::to_owned).or(gpu_error.map(|g| g.to_string())) {
                failed.push(format!("{id}: {}", err.lines().next().unwrap_or_default()));
            } else if !m.iter().all(|c| c.is_finite()) {
                failed.push(format!("{id}: not finite"));
            }
        }
        set_library(openvj_core::EffectLibrary::builtin());
        assert!(failed.is_empty(), "{failed:#?}");
        eprintln!("{} generators and {} effects rendered", gens.len(), fxs.len());
    }

    #[test]
    fn every_builtin_effect_compiles_and_runs() {
        let Some(mut e) = engine(32, 32) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let (mut comp, _) = split_comp(&mut e);
        e.render(&comp, 0.1, &inputs(0.1), None);
        let plain = mean(&e.read_target(e.output()));
        for t in openvj_core::EffectLibrary::builtin().all() {
            comp.layers[0].effects = vec![Effect::new(t.id())];
            for frame in 0..3 {
                let now = 0.1 + frame as f64 / 60.0;
                e.render(&comp, now, &inputs(now), None);
            }
            assert!(e.effect_error(t.id()).is_none(), "{}: {}", t.id(), e.effect_error(t.id()).unwrap());
            let m = mean(&e.read_target(e.output()));
            assert!(m.iter().all(|c| c.is_finite()), "{}: {m:?}", t.id());
            if t.id() == "invert" {
                assert!((m[0] - (255.0 - plain[0])).abs() < 2.0, "invert: {m:?} vs {plain:?}");
            }
        }
    }

    #[test]
    fn feedback_effects_keep_state_between_frames() {
        let Some(mut e) = engine(32, 32) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let (mut comp, id) = split_comp(&mut e);
        let mut fx = Effect::new("acuarela");
        *fx.param_mut("gain").unwrap() = openvj_core::Param::new(0.0);
        comp.layers[0].effects.push(fx);
        e.render(&comp, 0.0, &inputs(0.0), None);
        let first = mean(&e.read_target(e.output()));
        assert!((first[0] - 127.5).abs() < 10.0, "acuarela starts from the input: {first:?}");

        // Swap in a black frame: the white half lingers through the feedback, fading at 98%.
        e.set_media_frame(id, 1, 1, &[0, 0, 0, 255]);
        e.render(&comp, 1.0 / 60.0, &inputs(1.0 / 60.0), None);
        let trail = mean(&e.read_target(e.output()));
        assert!((trail[0] - first[0] * 0.98).abs() < 6.0, "trail: {trail:?} after {first:?}");

        // Firing Reset clears the feedback.
        let fx = &mut comp.layers[0].effects[0];
        fx.param_mut("reset").unwrap().value = 1.0;
        e.render(&comp, 2.0 / 60.0, &inputs(2.0 / 60.0), None);
        assert!(mean(&e.read_target(e.output()))[0] < 1.0);
    }
}
