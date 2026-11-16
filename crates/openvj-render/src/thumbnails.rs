//! Library thumbnails, rendered in the background while the show runs.
//!
//! [`Engine::request_thumbnail`] queues a source. Each [`Engine::render`] advances the job in
//! front of the queue by one small step (one 256×144 draw), so a thumbnail never costs more
//! than a sliver of a frame: generators and MilkDrop presets run [`THUMB_SECONDS`] of time in
//! [`THUMB_STEPS`] steps on a quiet synthetic beat, so feedback shaders have time to build up,
//! and the last step is copied out without blocking. [`Engine::take_thumbnails`] hands back
//! finished pixels. Effects render on [`effect_test_source`], a built-in test pattern.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use openvj_core::{AudioSignal, Clip, ClipId, ClipSource, Composition, Effect, GeneratorKind, ModInputs};

use crate::{Draw, Engine, LDR, MODE_MIX, Pass, Target, Targets};

/// Size of library thumbnails.
pub const LIBRARY_THUMB_SIZE: (u32, u32) = (256, 144);

/// Seconds of time a generator runs before its thumbnail is taken.
pub const THUMB_SECONDS: f64 = 3.0;

/// Draws per thumbnail (one per output frame).
pub const THUMB_STEPS: u32 = 8;

/// Frames a job may wait for a source to show anything (a MilkDrop preset loading) before it
/// gives up.
const MAX_WAIT_FRAMES: u32 = 240;

/// Tempo of the synthetic beat, in beats per minute.
const THUMB_BPM: f64 = 120.0;

/// Clip ids of thumbnail jobs count down from here, far from compositions' and the Player's.
const FIRST_JOB_ID: u64 = u64::MAX - 1;

/// A finished thumbnail.
#[derive(Debug, Clone, PartialEq)]
pub struct Thumbnail {
    /// The key it was requested with.
    pub key: String,
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA8, or `None` when the source never drew anything.
    pub rgba: Option<Vec<u8>>,
}

/// What a thumbnail shows.
#[derive(Debug, Clone)]
pub enum ThumbnailSource {
    /// A clip (generator, ISF file, MilkDrop preset or saved preset) with its parameter
    /// values and effects. Make one with [`thumbnail_clip`].
    Clip(Box<Clip>),
    /// The Preview monitor as it is now ("Use Preview frame as thumbnail").
    Preview,
}

/// The test pattern effects are shown on: a built-in generator with colour, edges and motion.
pub fn effect_test_source() -> ClipSource {
    ClipSource::Generator { kind: GeneratorKind::Kaleido }
}

struct Job {
    key: String,
    clip: Option<Clip>,
    step: u32,
    waited: u32,
}

struct Readback {
    key: String,
    buffer: wgpu::Buffer,
    ready: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
}

/// The engine's thumbnail queue.
#[derive(Default)]
pub(crate) struct Thumbnails {
    queue: VecDeque<(String, ThumbnailSource)>,
    job: Option<Job>,
    target: Option<Target>,
    readbacks: Vec<Readback>,
    done: Vec<Thumbnail>,
    paused: bool,
    next_id: u64,
}

impl Thumbnails {
    /// The clip id of the job in progress, so its MilkDrop engine isn't dropped.
    pub(crate) fn job_clip(&self) -> Option<ClipId> {
        self.job.as_ref().and_then(|j| j.clip.as_ref()).map(|c| c.id)
    }
}

/// A standalone clip of a source with default settings and `effects` on it.
pub fn thumbnail_clip(source: ClipSource, effects: Vec<Effect>) -> ThumbnailSource {
    let mut comp = Composition::new(16, 9, 1, 1);
    comp.set_clip(0, 0, source);
    let mut clip = comp.clip(0, 0).cloned().expect("slot 0,0 exists");
    clip.effects = effects;
    ThumbnailSource::Clip(Box::new(clip))
}

/// A quiet, steady beat: low levels that pulse a little on each beat.
fn synthetic_inputs(time: f64) -> ModInputs {
    let beat = time * THUMB_BPM / 60.0;
    let pulse = (1.0 - beat.fract() as f32).powi(4);
    let level = |base: f32| base + 0.15 * pulse;
    let bands = [level(0.3), level(0.2), level(0.15), level(0.1)];
    let signal = |l: f32| AudioSignal { level: l, hits: pulse * 0.5, presence: 0.3, clock: time as f32 };
    ModInputs { beat, bands, band_signals: bands.map(signal), overall: signal(level(0.2)) }
}

/// A quiet sine with a kick on the beat, as MilkDrop's audio for one frame.
fn synthetic_pcm(time: f64, rate: u32) -> Vec<f32> {
    let n = (rate as f64 / 60.0) as usize;
    let beat_len = 60.0 / THUMB_BPM;
    (0..n)
        .map(|i| {
            let t = time + i as f64 / rate as f64;
            let kick = (-(t % beat_len) * 12.0).exp() * (t * 60.0 * std::f64::consts::TAU).sin();
            (0.1 * (t * 440.0 * std::f64::consts::TAU).sin() + 0.3 * kick) as f32
        })
        .collect()
}

impl Engine {
    /// Queues a thumbnail. `key` comes back with the result in [`Engine::take_thumbnails`].
    pub fn request_thumbnail(&mut self, key: impl Into<String>, source: ThumbnailSource) {
        self.thumbnails.queue.push_back((key.into(), source));
    }

    /// Jobs queued or in progress (not counting finished ones not yet taken).
    pub fn thumbnails_pending(&self) -> usize {
        self.thumbnails.queue.len() + usize::from(self.thumbnails.job.is_some())
    }

    /// Drops every queued job (the one in progress finishes).
    pub fn clear_thumbnail_queue(&mut self) {
        self.thumbnails.queue.clear();
    }

    /// Stops or resumes thumbnail work, e.g. while the output is over its frame budget.
    pub fn pause_thumbnails(&mut self, paused: bool) {
        self.thumbnails.paused = paused;
    }

    /// Thumbnails finished since the last call. Never blocks.
    pub fn take_thumbnails(&mut self) -> Vec<Thumbnail> {
        if !self.thumbnails.readbacks.is_empty() {
            let _ = self.device.poll(wgpu::PollType::Poll);
        }
        let (w, h) = LIBRARY_THUMB_SIZE;
        let mut still = Vec::new();
        for rb in std::mem::take(&mut self.thumbnails.readbacks) {
            if rb.failed.load(Ordering::Acquire) {
                self.thumbnails.done.push(Thumbnail { key: rb.key, width: w, height: h, rgba: None });
            } else if rb.ready.load(Ordering::Acquire) {
                let rgba = rb.buffer.slice(..).get_mapped_range().map(|m| m.to_vec()).ok();
                rb.buffer.unmap();
                self.thumbnails.done.push(Thumbnail { key: rb.key, width: w, height: h, rgba });
            } else {
                still.push(rb);
            }
        }
        self.thumbnails.readbacks = still;
        std::mem::take(&mut self.thumbnails.done)
    }

    fn thumb_target(&mut self) -> Target {
        let device = &self.device;
        let (w, h) = LIBRARY_THUMB_SIZE;
        self.thumbnails.target.get_or_insert_with(|| Target::new(device, "library thumb", w, h, LDR)).clone()
    }

    /// One step of the job in front of the queue, recorded into this frame's passes. Returns
    /// the key when the job's last draw is in `passes`, to be read back after submitting.
    pub(crate) fn thumbnail_step(&mut self, passes: &mut Vec<Pass>, t: &Targets) -> Option<String> {
        if self.thumbnails.paused {
            return None;
        }
        if self.thumbnails.job.is_none() {
            let (key, source) = self.thumbnails.queue.pop_front()?;
            let clip = match source {
                ThumbnailSource::Clip(mut clip) => {
                    self.thumbnails.next_id += 1;
                    clip.id = ClipId(FIRST_JOB_ID - self.thumbnails.next_id);
                    Some(*clip)
                }
                ThumbnailSource::Preview => None,
            };
            self.thumbnails.job = Some(Job { key, clip, step: 0, waited: 0 });
        }
        let target = self.thumb_target();
        let job = self.thumbnails.job.take().expect("set above");
        let Some(clip) = job.clip.clone() else {
            // The Preview, as drawn earlier this frame.
            passes.push(Self::blend_pass(&target, &t.preview, &t.preview, MODE_MIX, 0.0));
            return Some(job.key);
        };
        let time = THUMB_SECONDS * (job.step + 1) as f64 / THUMB_STEPS as f64;
        let inputs = synthetic_inputs(time);
        let live_pcm = std::mem::replace(&mut self.pcm, synthetic_pcm(time, self.pcm_rate));
        let drawn = passes.len();
        self.clip_passes(passes, &t.fx, &clip, time, &inputs, &target, 3);
        self.pcm = live_pcm;
        let showed = !matches!(passes.last(), Some(p) if passes.len() == drawn + 1 && matches!(p.draw, Draw::Clear));
        if !clip.effects.is_empty() {
            let out = self.effect_chain(passes, &t.fx, target.clone(), &clip.effects, time, &inputs);
            if out.id != target.id {
                passes.push(Self::blend_pass(&target, &out, &out, MODE_MIX, 0.0));
            }
        }
        let mut job = job;
        if showed {
            job.step += 1;
        } else {
            job.waited += 1;
        }
        if job.step >= THUMB_STEPS {
            return Some(job.key);
        }
        if job.waited >= MAX_WAIT_FRAMES {
            let (w, h) = LIBRARY_THUMB_SIZE;
            self.thumbnails.done.push(Thumbnail { key: job.key, width: w, height: h, rgba: None });
            return None;
        }
        self.thumbnails.job = Some(job);
        None
    }

    /// Starts copying the finished thumbnail to the CPU (after this frame was submitted).
    pub(crate) fn thumbnail_readback(&mut self, key: String) {
        let target = self.thumb_target();
        let (w, h) = (target.width, target.height);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("library thumb readback"),
            size: (w * 4 * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            target.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                // 256 px × 4 bytes is already a multiple of wgpu's 256-byte row alignment.
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: None },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        let ready = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let (r, f) = (ready.clone(), failed.clone());
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |res| match res {
            Ok(()) => r.store(true, Ordering::Release),
            Err(_) => f.store(true, Ordering::Release),
        });
        self.thumbnails.readbacks.push(Readback { key, buffer, ready, failed });
    }
}

const _: () = assert!((LIBRARY_THUMB_SIZE.0 * 4).is_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> Option<Engine> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .ok()?;
        Some(Engine::new(device, queue, 320, 180))
    }

    /// Renders frames until `n` thumbnails are back (or gives up).
    fn run(e: &mut Engine, comp: &Composition, n: usize) -> (Vec<Thumbnail>, u32) {
        let mut out = Vec::new();
        for frame in 0..400u32 {
            let now = frame as f64 / 60.0;
            e.render(comp, now, &ModInputs::default(), None);
            let _ = e.device().poll(wgpu::PollType::wait_indefinitely());
            out.extend(e.take_thumbnails());
            if out.len() >= n {
                return (out, frame + 1);
            }
        }
        (out, 400)
    }

    fn lit(t: &Thumbnail) -> bool {
        let px = t.rgba.as_ref().expect("pixels");
        assert_eq!(px.len(), (t.width * t.height * 4) as usize);
        px.chunks(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30).count() > px.len() / 4 / 10
    }

    #[test]
    fn renders_generators_and_effects_a_step_per_frame() {
        let Some(mut e) = engine() else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let comp = Composition::new(320, 180, 1, 1);
        let plasma = ClipSource::Generator { kind: GeneratorKind::Plasma };
        e.request_thumbnail("gen:plasma", thumbnail_clip(plasma, Vec::new()));
        e.request_thumbnail("isf:blur", thumbnail_clip(effect_test_source(), vec![Effect::new("blur")]));
        e.request_thumbnail("isf:missing", thumbnail_clip(ClipSource::isf("missing"), Vec::new()));
        assert_eq!(e.thumbnails_pending(), 3);

        let (done, frames) = run(&mut e, &comp, 2);
        assert_eq!(done.iter().map(|t| t.key.as_str()).collect::<Vec<_>>(), ["gen:plasma", "isf:blur"]);
        assert!(frames >= 2 * THUMB_STEPS, "one step per frame, took {frames}");
        assert!(done.iter().all(lit));
        assert_eq!((done[0].width, done[0].height), LIBRARY_THUMB_SIZE);
    }

    #[test]
    fn paused_queue_waits_and_preview_frames_can_be_kept() {
        let Some(mut e) = engine() else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let mut comp = Composition::new(320, 180, 1, 1);
        let id = comp.set_clip(0, 0, ClipSource::Generator { kind: GeneratorKind::Gradient }).unwrap();
        e.request_thumbnail("preview", ThumbnailSource::Preview);
        e.pause_thumbnails(true);
        for i in 0..5 {
            e.render(&comp, i as f64 / 60.0, &ModInputs::default(), Some(id));
        }
        assert_eq!(e.thumbnails_pending(), 1);
        e.pause_thumbnails(false);
        e.render(&comp, 0.1, &ModInputs::default(), Some(id));
        let _ = e.device().poll(wgpu::PollType::wait_indefinitely());
        let done = e.take_thumbnails();
        assert_eq!(done.len(), 1);
        assert!(lit(&done[0]));
    }

    #[test]
    fn synthetic_audio_is_quiet_but_pulses() {
        let on = synthetic_inputs(0.0);
        let off = synthetic_inputs(0.4);
        assert!(on.bands[0] > off.bands[0] && on.bands[0] < 0.5);
        assert!(synthetic_pcm(0.0, 48_000).iter().all(|s| s.abs() <= 0.4));
    }
    #[test]
    fn auditions_on_the_preview_without_touching_the_output() {
        let Some(mut e) = engine() else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let comp = Composition::new(320, 180, 1, 1);
        let ThumbnailSource::Clip(mut clip) =
            thumbnail_clip(ClipSource::Generator { kind: GeneratorKind::Plasma }, vec![])
        else {
            unreachable!()
        };
        clip.id = crate::AUDITION_CLIP;
        e.audition(Some(*clip));
        assert!(e.auditioning());
        e.render(&comp, 0.5, &ModInputs::default(), None);
        let lit_px = |px: Vec<u8>| px.chunks(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30).count();
        assert!(lit_px(e.read_target(e.preview())) > 1000, "preview shows the auditioned clip");
        assert_eq!(lit_px(e.read_target(e.output())), 0, "output stays black");
        e.audition(None);
        e.render(&comp, 0.6, &ModInputs::default(), None);
        assert_eq!(lit_px(e.read_target(e.preview())), 0);
    }
}
