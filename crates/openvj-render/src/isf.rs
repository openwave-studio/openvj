//! Compiles ISF effects (see `openvj_core::isf`) into wgpu pipelines.
//!
//! Standard ISF GLSL gets a generated preamble: the ISF uniforms, `inputImage`, one texture
//! per pass target, the inputs as a uniform block and the `IMG_*` macros. A file that brings
//! its own `#version` line (Varda-style, with explicit `layout(set = 0, binding = n)`) is
//! used as written. Either way the GLSL goes through naga's GLSL front end, and bindings are
//! matched by *name* from the compiled module, so both styles fill the same way.

use std::borrow::Cow;
use std::collections::HashMap;

use openvj_core::isf::{EffectDef, InputType};

/// What a uniform block member is, for packing.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Scalar {
    F32,
    I32,
    U32,
}

#[derive(Debug, Clone)]
struct Member {
    name: String,
    offset: u32,
    scalar: Scalar,
    /// 1 to 4 components.
    width: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct UniformBlock {
    pub binding: u32,
    pub size: u32,
    members: Vec<Member>,
}

/// A compiled effect: one pipeline shared by every pass (passes differ by `PASSINDEX`).
pub(crate) struct CompiledEffect {
    pub pipeline: wgpu::RenderPipeline,
    pub layout: wgpu::BindGroupLayout,
    pub blocks: Vec<UniformBlock>,
    /// (binding, texture name).
    pub textures: Vec<(u32, String)>,
    pub sampler: Option<u32>,
}

/// Prefix of the int members standing in for bool inputs (bools can't live in uniform blocks).
const BOOL_PREFIX: &str = "isf_b_";

/// Unique pass target names, in order of first appearance.
pub(crate) fn target_names(def: &EffectDef) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in &def.passes {
        if let Some(t) = &p.target
            && !out.contains(t)
        {
            out.push(t.clone());
        }
    }
    out
}

/// The GLSL handed to naga.
pub(crate) fn glsl_source(def: &EffectDef) -> String {
    if def.glsl.contains("#version") {
        return def.glsl.clone();
    }
    let mut s = String::from(
        "#version 450\n\
         layout(set = 0, binding = 0) uniform ISF_Globals {\n\
         \x20   float TIME; float TIMEDELTA; int FRAMEINDEX; int PASSINDEX;\n\
         \x20   vec2 RENDERSIZE; float BEAT; float BPM;\n\
         \x20   vec4 DATE; vec4 AUDIO;\n\
         };\n\
         layout(set = 0, binding = 1) uniform sampler isf_sampler;\n\
         layout(set = 0, binding = 2) uniform texture2D inputImage;\n",
    );
    let mut binding = 3;
    for t in target_names(def) {
        s += &format!("layout(set = 0, binding = {binding}) uniform texture2D {t};\n");
        binding += 1;
    }
    let mut members = String::new();
    let mut defines = String::new();
    for input in &def.inputs {
        let n = &input.name;
        match input.ty {
            InputType::Float { .. } => members += &format!("    float {n};\n"),
            InputType::Long { .. } => members += &format!("    int {n};\n"),
            InputType::Bool { .. } | InputType::Event => {
                members += &format!("    int {BOOL_PREFIX}{n};\n");
                defines += &format!("#define {n} ({BOOL_PREFIX}{n} != 0)\n");
            }
            InputType::Color { .. } => members += &format!("    vec4 {n};\n"),
            InputType::Point2D { .. } => members += &format!("    vec2 {n};\n"),
            InputType::Image => {}
        }
    }
    if !members.is_empty() {
        s += &format!("layout(set = 0, binding = {binding}) uniform ISF_Inputs {{\n{members}}};\n");
    }
    s += &defines;
    s += "layout(location = 0) in vec2 isf_FragNormCoord;\n\
          layout(location = 0) out vec4 isf_FragColor;\n\
          #define gl_FragColor isf_FragColor\n\
          #define vv_FragNormCoord isf_FragNormCoord\n\
          #define IMG_NORM_PIXEL(img, c) texture(sampler2D(img, isf_sampler), c)\n\
          #define IMG_THIS_PIXEL(img) IMG_NORM_PIXEL(img, isf_FragNormCoord)\n\
          #define IMG_THIS_NORM_PIXEL(img) IMG_NORM_PIXEL(img, isf_FragNormCoord)\n\
          #define IMG_SIZE(img) vec2(textureSize(sampler2D(img, isf_sampler), 0))\n\
          #define IMG_PIXEL(img, c) IMG_NORM_PIXEL(img, (c) / IMG_SIZE(img))\n\
          #define IMG_THIS_PIXEL_RECT(img) IMG_THIS_PIXEL(img)\n\
          #define texture2D(img, c) IMG_NORM_PIXEL(img, c)\n\
          #define renderSize RENDERSIZE\n";
    s += &ghost_arcade_shim(def);
    s += "#line 1\n";
    s += &without_redeclared_builtins(&def.glsl);
    s
}

/// Ghost Arcade's built-in audio names, mapped onto openVJ's four bands, beat and tempo.
const GHOST_ARCADE_NAMES: &[(&str, &str)] = &[
    ("audioBass", "AUDIO.x"),
    ("audioMid", "((AUDIO.y + AUDIO.z) * 0.5)"),
    ("audioHigh", "AUDIO.w"),
    ("audioLevel", "dot(AUDIO, vec4(0.25))"),
    ("audioBeatPhase", "fract(BEAT)"),
    ("audioBeat", "(1.0 - fract(BEAT))"),
    ("audioBPM", "BPM"),
    ("audioSpectralCentroid", "dot(AUDIO, vec4(0.125, 0.375, 0.625, 0.875)) / max(dot(AUDIO, vec4(1.0)), 1e-4)"),
];

/// Defines for the Ghost Arcade names a file uses but doesn't declare itself as an input.
fn ghost_arcade_shim(def: &EffectDef) -> String {
    let glsl = &def.glsl;
    let mut s = String::new();
    for (name, expr) in GHOST_ARCADE_NAMES {
        if uses_word(glsl, name) && !def.inputs.iter().any(|i| i.name == *name) {
            s += &format!("#define {name} {expr}\n");
        }
    }
    // WebGL ISF hosts allow these GLSL ES 1.0 non-keywords as names; GLSL 450 doesn't.
    for word in ["centroid", "sample", "patch", "buffer", "filter", "input", "output"] {
        if uses_word(glsl, word) {
            s += &format!("#define {word} isf_{word}\n");
        }
    }
    if uses_word(glsl, "sampleFFT") && !glsl.contains("float sampleFFT(") {
        // No FFT texture yet: step through the four bands.
        s += "float sampleFFT(float u) { return AUDIO[int(clamp(u, 0.0, 0.999) * 4.0)]; }\n";
    }
    if uses_word(glsl, "sampleWaveform") && !glsl.contains("float sampleWaveform(") {
        s += "float sampleWaveform(float u) { return 0.0; }\n";
    }
    s
}

/// Whether `word` appears in `src` as a whole identifier.
fn uses_word(src: &str, word: &str) -> bool {
    src.match_indices(word).any(|(i, _)| {
        let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
        !src[..i].ends_with(ident) && !src[i + word.len()..].starts_with(ident)
    })
}

/// Blanks out `precision` lines and plain `uniform` lines that redeclare a name openVJ already
/// provides (`uniform float TIME;`, Ghost Arcade's `uniform float audioBass;`), which WebGL ISF
/// hosts tolerate. Lines stay so compiler errors keep their line numbers.
fn without_redeclared_builtins(glsl: &str) -> String {
    const PROVIDED: &[&str] = &["TIME", "TIMEDELTA", "FRAMEINDEX", "PASSINDEX", "RENDERSIZE", "renderSize", "DATE"];
    let mut out = String::with_capacity(glsl.len());
    for line in glsl.lines() {
        let t = line.trim();
        let redeclared = t.starts_with("uniform ")
            && t.ends_with(';')
            && t.trim_end_matches(';')
                .split_whitespace()
                .last()
                .is_some_and(|n| PROVIDED.contains(&n) || GHOST_ARCADE_NAMES.iter().any(|(g, _)| *g == n));
        if !redeclared && !t.starts_with("precision ") {
            out += line;
        }
        out.push('\n');
    }
    out
}

/// Parses and validates the GLSL. The error is a readable compiler message.
pub(crate) fn parse(def: &EffectDef) -> Result<naga::Module, String> {
    let src = glsl_source(def);
    let module = naga::front::glsl::Frontend::default()
        .parse(&naga::front::glsl::Options::from(naga::ShaderStage::Fragment), &src)
        .map_err(|e| e.emit_to_string(&src))?;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&module)
        .map_err(|e| e.emit_to_string(&src))?;
    Ok(module)
}

/// The bindings a compiled module uses.
struct Reflection {
    blocks: Vec<UniformBlock>,
    textures: Vec<(u32, String)>,
    sampler: Option<u32>,
}

fn reflect(module: &naga::Module) -> Result<Reflection, String> {
    let mut blocks = Vec::new();
    let mut textures = Vec::new();
    let mut sampler = None;
    for (_, var) in module.global_variables.iter() {
        let Some(rb) = &var.binding else { continue };
        if rb.group != 0 {
            return Err(format!("{}: only descriptor set 0 is supported", var.name.as_deref().unwrap_or("?")));
        }
        let ty = &module.types[var.ty];
        match (&var.space, &ty.inner) {
            (naga::AddressSpace::Uniform, naga::TypeInner::Struct { members, span }) => {
                let mut out = Vec::new();
                for m in members {
                    let (scalar, width) = match module.types[m.ty].inner {
                        naga::TypeInner::Scalar(s) => (s, 1),
                        naga::TypeInner::Vector { size, scalar } => (scalar, size as u32),
                        _ => continue,
                    };
                    let scalar = match scalar.kind {
                        naga::ScalarKind::Float => Scalar::F32,
                        naga::ScalarKind::Sint => Scalar::I32,
                        naga::ScalarKind::Uint | naga::ScalarKind::Bool => Scalar::U32,
                        _ => continue,
                    };
                    if let Some(name) = &m.name {
                        out.push(Member { name: name.clone(), offset: m.offset, scalar, width });
                    }
                }
                blocks.push(UniformBlock { binding: rb.binding, size: *span, members: out });
            }
            (_, naga::TypeInner::Image { .. }) => {
                textures.push((rb.binding, var.name.clone().unwrap_or_default()));
            }
            (_, naga::TypeInner::Sampler { .. }) => sampler = Some(rb.binding),
            _ => {}
        }
    }
    Ok(Reflection { blocks, textures, sampler })
}

pub(crate) fn compile(
    device: &wgpu::Device,
    vertex: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    def: &EffectDef,
) -> Result<CompiledEffect, String> {
    let module = parse(def)?;
    let Reflection { blocks, textures, sampler } = reflect(&module)?;
    let mut entries = Vec::new();
    for b in &blocks {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: b.binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(b.size as u64),
            },
            count: None,
        });
    }
    for (binding, _) in &textures {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: *binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
    }
    if let Some(binding) = sampler {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
    }
    let layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(&def.id), entries: &entries });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&def.id),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let fragment = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(&def.id),
        source: wgpu::ShaderSource::Naga(Cow::Owned(module)),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(&def.id),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: vertex,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &fragment,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
        }),
        multiview_mask: None,
        cache: None,
    });
    Ok(CompiledEffect { pipeline, layout, blocks, textures, sampler })
}

/// Named values for one pass; anything a block asks for that isn't here is zero.
#[derive(Default)]
pub(crate) struct Values(HashMap<String, [f32; 4]>);

impl Values {
    pub fn set(&mut self, name: &str, v: &[f32]) {
        let mut a = [0.0; 4];
        a[..v.len().min(4)].copy_from_slice(&v[..v.len().min(4)]);
        self.0.insert(name.to_owned(), a);
    }

    fn get(&self, name: &str) -> Option<&[f32; 4]> {
        self.0.get(name).or_else(|| self.0.get(name.strip_prefix(BOOL_PREFIX)?))
    }

    /// Packs a uniform block.
    pub fn pack(&self, block: &UniformBlock, out: &mut [u8]) {
        for m in &block.members {
            let Some(v) = self.get(&m.name) else { continue };
            for (c, x) in v.iter().enumerate().take(m.width as usize) {
                let at = m.offset as usize + c * 4;
                let bytes = match m.scalar {
                    Scalar::F32 => x.to_le_bytes(),
                    Scalar::I32 => (x.round() as i32).to_le_bytes(),
                    Scalar::U32 => (x.max(0.0).round() as u32).to_le_bytes(),
                };
                if let Some(dst) = out.get_mut(at..at + 4) {
                    dst.copy_from_slice(&bytes);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openvj_core::library;

    #[test]
    fn every_builtin_effect_compiles() {
        for t in library().all() {
            let module = parse(&t.def).unwrap_or_else(|e| panic!("{}:\n{e}", t.id()));
            let Reflection { blocks, textures, sampler } = reflect(&module).unwrap();
            assert!(sampler.is_some(), "{}", t.id());
            assert!(textures.iter().any(|(_, n)| n == "inputImage"), "{}", t.id());
            let inputs = blocks.iter().find(|b| b.members.iter().any(|m| m.name != "TIME")).map(|b| b.members.len());
            let expected = t.def.inputs.iter().filter(|i| i.ty != InputType::Image).count();
            if expected > 0 {
                assert!(blocks.len() == 2 && inputs.is_some(), "{}: {blocks:?}", t.id());
            }
        }
    }

    #[test]
    fn varda_style_files_and_bools_pack_by_name() {
        let src = r#"/*{ "INPUTS": [ {"NAME": "inputImage", "TYPE": "image"},
                {"NAME": "amount", "TYPE": "float"}, {"NAME": "on", "TYPE": "bool"} ] }*/
            #version 450
            layout(location = 0) out vec4 fragColor;
            layout(location = 0) in vec2 uv;
            layout(set = 0, binding = 0) uniform ISFUniforms { float TIME; vec2 RENDERSIZE; };
            layout(set = 0, binding = 1) uniform sampler samp;
            layout(set = 0, binding = 2) uniform texture2D inputImage;
            layout(set = 0, binding = 3) uniform P { float amount; int on; };
            void main() { fragColor = texture(sampler2D(inputImage, samp), uv) * amount * float(on) + TIME; }"#;
        let def = EffectDef::parse(src, "varda").unwrap();
        let Reflection { blocks, textures, sampler } = reflect(&parse(&def).unwrap()).unwrap();
        assert_eq!(sampler, Some(1));
        assert_eq!(textures, [(2, "inputImage".to_owned())]);
        let p = blocks.iter().find(|b| b.binding == 3).unwrap();
        let mut v = Values::default();
        v.set("amount", &[0.5]);
        v.set("on", &[1.0]);
        let mut buf = vec![0u8; p.size as usize];
        v.pack(p, &mut buf);
        assert_eq!(f32::from_le_bytes(buf[0..4].try_into().unwrap()), 0.5);
        assert_eq!(i32::from_le_bytes(buf[4..8].try_into().unwrap()), 1);
    }

    #[test]
    fn ghost_arcade_names_compile() {
        let src = r#"/*{ "INPUTS": [ {"NAME": "inputImage", "TYPE": "image"} ] }*/
            precision highp float;
            uniform float TIME;
            uniform vec2 renderSize;
            uniform float audioBass;
            void main() {
                float audioMidish = audioMid + sampleFFT(0.1) + sampleWaveform(0.5) + audioBeatPhase;
                vec2 uv = gl_FragCoord.xy / renderSize;
                gl_FragColor = IMG_THIS_PIXEL(inputImage) * (audioBass + audioLevel + audioMidish + uv.x);
            }"#;
        let def = EffectDef::parse(src, "ga").unwrap();
        parse(&def).unwrap_or_else(|e| panic!("{e}"));
        assert!(uses_word("x audioMid;", "audioMid") && !uses_word("audioMidish", "audioMid"));
    }

    #[test]
    fn every_bundled_shader_compiles() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shaders");
        let r = openvj_core::effect_folders::Scanner::new(vec![dir], crate::check_effect).scan().unwrap();
        let errors: Vec<_> = r.errors.iter().map(|e| format!("{}: {}", e.path.display(), e.message)).collect();
        assert!(errors.is_empty(), "{errors:#?}");
        assert!(r.generators.len() > 300 && r.effects.len() > 10, "{} / {}", r.generators.len(), r.effects.len());
    }

    #[test]
    fn compile_errors_are_readable() {
        let def = EffectDef::parse(r#"/*{ "INPUTS": [] }*/ void main() { gl_FragColor = nope; }"#, "bad").unwrap();
        let err = parse(&def).err().unwrap();
        assert!(err.contains("nope"), "{err}");
    }

    #[test]
    fn readme_example_compiles() {
        let readme = include_str!("../../../README.md");
        let start = readme.find("```glsl\n").expect("README has a GLSL example") + 8;
        let src = &readme[start..start + readme[start..].find("```").unwrap()];
        let def = EffectDef::parse(src, "readme").unwrap();
        parse(&def).unwrap_or_else(|e| panic!("{e}"));
    }
}
