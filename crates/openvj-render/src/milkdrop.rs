//! MilkDrop presets, rendered by [lactea](https://github.com/M0Rf30/lactea).
//!
//! Each MilkDrop clip owns one lactea engine. It renders at most once per frame into the
//! clip's own `Rgba16Float` texture, recorded into openVJ's frame command buffer; the
//! layer, preview and thumbnail copy from it. A clip seen only as a thumbnail runs a small
//! engine at thumbnail size, so a grid full of presets stays cheap.

use std::path::PathBuf;
use std::sync::Arc;

use openvj_core::milkdrop::MilkPreset;

use crate::{HDR, THUMB_SIZE, Target};

/// Seconds a clip keeps its full-size engine after it was last on a layer or in the preview.
pub(crate) const FULL_HOLD_SECS: f64 = 2.0;
/// Warp mesh cells of full-size and thumbnail engines (MilkDrop's default is 48×36).
const FULL_MESH: [u32; 2] = [48, 36];
const THUMB_MESH: [u32; 2] = [24, 18];

/// GPU state of one MilkDrop clip.
pub(crate) struct MilkClip {
    engine: lactea::Engine,
    pub target: Target,
    /// The preset loaded: (id, file version).
    loaded: Option<(String, u64)>,
    /// Why the preset didn't load.
    pub error: Option<String>,
    sample_rate: u32,
    pub full: bool,
    /// Composition time it was last drawn full size.
    pub last_full: f64,
    /// [`crate::Engine`] frame number it last rendered in.
    pub rendered: u64,
    frame: u32,
}

impl MilkClip {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: (u32, u32),
        full: bool,
        sample_rate: u32,
    ) -> Result<Self, String> {
        let (w, h) = if full { size } else { THUMB_SIZE };
        let cfg = lactea::EngineConfig {
            width: w,
            height: h,
            mesh: if full { FULL_MESH } else { THUMB_MESH },
            output_format: HDR,
            sample_rate: sample_rate.max(1),
            // Presets written for MilkDrop 2 or 3 load as such; the rest as projectM plays them.
            auto_compat: true,
            ..Default::default()
        };
        let mut engine = lactea::Engine::new(device, queue, cfg).map_err(|e| e.to_string())?;
        engine.set_texture_provider(Box::new(FolderTextures::new()));
        Ok(Self {
            engine,
            target: Target::new(device, "milkdrop", w, h, HDR),
            loaded: None,
            error: None,
            sample_rate,
            full,
            last_full: f64::NEG_INFINITY,
            rendered: 0,
            frame: 0,
        })
    }

    /// Whether this engine fits a request (full size or thumbnail, at this size and rate).
    pub fn fits(&self, size: (u32, u32), full: bool, sample_rate: u32) -> bool {
        let want = if full { size } else { THUMB_SIZE };
        self.full == full && (self.target.width, self.target.height) == want && self.sample_rate == sample_rate
    }

    /// Loads `preset` unless it's already loaded. Errors stay in [`MilkClip::error`].
    pub fn load(&mut self, preset: &MilkPreset) {
        let key = (preset.id.clone(), preset.version);
        if self.loaded.as_ref() == Some(&key) {
            return;
        }
        self.loaded = Some(key);
        self.error = match std::fs::read(&preset.path) {
            Ok(bytes) => {
                self.engine.load_preset(&bytes, &preset.name, lactea::Transition::Cut).err().map(|e| e.to_string())
            }
            Err(e) => Some(format!("can't read {}: {e}", preset.path.display())),
        };
    }

    pub fn is_ready(&self) -> bool {
        self.loaded.is_some() && self.error.is_none()
    }

    /// Hands this frame's mono samples (−1..1) to the preset, scaled by `gain`.
    pub fn push_audio(&mut self, samples: &[f32], gain: f32) {
        if samples.is_empty() {
            return;
        }
        if (gain - 1.0).abs() < 1e-4 {
            self.engine.push_pcm_f32(samples, 1);
        } else {
            let scaled: Vec<f32> = samples.iter().map(|s| (s * gain).clamp(-1.0, 1.0)).collect();
            self.engine.push_pcm_f32(&scaled, 1);
        }
    }

    /// Records one frame at clip time `time` (seconds) into [`MilkClip::target`].
    pub fn render(&mut self, encoder: &mut wgpu::CommandEncoder, time: f64) {
        let clock = lactea::FrameClock { time, frame: self.frame };
        self.frame = self.frame.wrapping_add(1);
        let view = self.target.view.clone();
        self.engine.render(encoder, &view, clock);
    }
}

/// Textures presets ask for (`sampler_clouds2`), from a `textures` folder in any MilkDrop
/// folder, as projectM and MilkDrop keep them.
struct FolderTextures {
    files: Vec<(String, PathBuf)>,
}

impl FolderTextures {
    fn new() -> Self {
        let mut files = Vec::new();
        for dir in openvj_core::milkdrop::milk_library().dirs.iter().map(|d| d.join("textures")) {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for path in entries.flatten().map(|e| e.path()) {
                let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
                if matches!(ext.as_deref(), Some("jpg" | "jpeg" | "png" | "bmp" | "tga" | "dds" | "gif"))
                    && let Some(stem) = path.file_stem()
                {
                    files.push((stem.to_string_lossy().to_lowercase(), path));
                }
            }
        }
        files.sort();
        Self { files }
    }
}

impl lactea::TextureProvider for FolderTextures {
    fn load(&mut self, name: &str) -> Option<lactea::Image> {
        let (_, path) = self.files.iter().find(|(n, _)| n == name)?;
        let img = image::open(path).ok()?.into_rgba8();
        Some(lactea::Image { width: img.width(), height: img.height(), rgba8: img.into_raw() })
    }

    fn random_pool(&self) -> Vec<String> {
        self.files.iter().map(|(n, _)| n.clone()).collect()
    }
}

/// The MilkDrop preset a clip plays, if it can be found.
pub(crate) fn resolve(id: &str) -> Option<Arc<MilkPreset>> {
    openvj_core::milkdrop::milk_library().resolve(id)
}
