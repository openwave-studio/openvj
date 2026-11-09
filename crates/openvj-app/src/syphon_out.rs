//! Syphon output: publishes the composition to other apps on macOS, as Resolume publishes
//! "Arena - Composition".
//!
//! Frames never leave the GPU. The engine keeps a BGRA copy of its output while a client is
//! connected (see `Engine::set_shared_output`), and Syphon's Metal server blits it into its
//! shared IOSurface on a command buffer from wgpu's own Metal queue. Metal runs command
//! buffers on one queue in commit order and tracks the texture, so the blit sees the
//! finished frame without any CPU wait.

use openvj_render::Engine;

/// True when this build can send Syphon (macOS with the `syphon` feature).
pub const AVAILABLE: bool = cfg!(all(feature = "syphon", target_os = "macos"));

/// The server name. Empty, so other apps list the server by the app's name alone ("OpenVJ").
#[cfg_attr(not(all(feature = "syphon", target_os = "macos")), allow(dead_code))]
pub const SERVER_NAME: &str = "";

/// The composition's Syphon server. Does nothing where Syphon isn't available.
pub struct SyphonOutput {
    #[cfg(all(feature = "syphon", target_os = "macos"))]
    gpu: (wgpu::Device, wgpu::Queue),
    #[cfg(all(feature = "syphon", target_os = "macos"))]
    server: Option<imp::Server>,
    /// The Settings switch. On by default; even then frames are only sent while an app receives.
    enabled: bool,
    /// Whether an app was receiving at the last frame.
    receiving: bool,
    /// Why there is no server, once creating one failed; it isn't retried.
    error: Option<String>,
}

impl SyphonOutput {
    #[cfg_attr(not(all(feature = "syphon", target_os = "macos")), allow(unused_variables))]
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            #[cfg(all(feature = "syphon", target_os = "macos"))]
            gpu: (device.clone(), queue.clone()),
            #[cfg(all(feature = "syphon", target_os = "macos"))]
            server: None,
            enabled: AVAILABLE,
            receiving: false,
            error: None,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Turns the server on or off. Off removes it, so other apps no longer list openVJ.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on && AVAILABLE;
        if !self.enabled {
            #[cfg(all(feature = "syphon", target_os = "macos"))]
            {
                self.server = None;
            }
            self.receiving = false;
        }
    }

    /// True while at least one app is receiving the composition.
    pub fn is_receiving(&self) -> bool {
        self.receiving
    }

    /// Why Syphon output couldn't start, if it couldn't.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Call before rendering a frame. Keeps the server at the composition size and turns the
    /// engine's shared output on only while some app is receiving.
    pub fn before_render(&mut self, engine: &mut Engine) {
        #[cfg(all(feature = "syphon", target_os = "macos"))]
        if self.enabled {
            let size = engine.size();
            if self.error.is_none() && self.server.as_ref().is_none_or(|s| s.size != size) {
                // Drop the old server first, so the new one can take the same name.
                self.server = None;
                match imp::Server::new(&self.gpu.0, SERVER_NAME, size) {
                    Ok(s) => self.server = Some(s),
                    Err(e) => {
                        eprintln!("Syphon output unavailable: {e}");
                        self.error = Some(e);
                    }
                }
            }
            self.receiving = self.server.as_ref().is_some_and(|s| s.has_clients());
        }
        engine.set_shared_output(self.receiving);
    }

    /// Call after rendering a frame: hands the frame to Syphon if anyone is receiving.
    #[cfg_attr(not(all(feature = "syphon", target_os = "macos")), allow(unused_variables))]
    pub fn after_render(&self, engine: &Engine) {
        #[cfg(all(feature = "syphon", target_os = "macos"))]
        if let (Some(server), Some(target)) = (&self.server, engine.shared_output()) {
            server.publish(&self.gpu.1, &target.texture);
        }
    }
}

#[cfg(all(feature = "syphon", target_os = "macos"))]
mod imp {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_metal::{MTLCommandBuffer, MTLCommandQueue};
    use wgpu::hal::api::Metal;

    pub struct Server {
        server: syphon_core::SyphonServer,
        pub size: (u32, u32),
    }

    impl Server {
        /// A Syphon Metal server on wgpu's own Metal device, so textures can be shared directly.
        pub fn new(device: &wgpu::Device, name: &str, size: (u32, u32)) -> Result<Self, String> {
            if !syphon_core::is_available() {
                return Err("Syphon.framework could not be loaded".into());
            }
            // SAFETY: the raw device is only passed to Syphon, which retains it.
            let hal = unsafe { device.as_hal::<Metal>() }.ok_or("the GPU isn't running on Metal")?;
            let raw: *mut AnyObject = Retained::as_ptr(hal.raw_device()).cast_mut().cast();
            let server = syphon_core::SyphonServer::new_with_name_and_device(name, raw, size.0, size.1)
                .map_err(|e| e.to_string())?;
            Ok(Self { server, size })
        }

        pub fn has_clients(&self) -> bool {
            self.server.has_clients()
        }

        /// Publishes `texture` (Bgra8Unorm, `size`, from the server's device). Syphon encodes
        /// a GPU copy on a new command buffer from wgpu's queue, committed after everything
        /// wgpu has submitted so far.
        pub fn publish(&self, queue: &wgpu::Queue, texture: &wgpu::Texture) {
            // SAFETY: the raw handles are used only within this call, on the thread that
            // owns the wgpu queue, and the command buffer is committed before returning.
            unsafe {
                let (Some(queue), Some(texture)) = (queue.as_hal::<Metal>(), texture.as_hal::<Metal>()) else {
                    return;
                };
                let Some(commands) = queue.as_raw().commandBuffer() else { return };
                let tex: *mut AnyObject = std::ptr::from_ref(texture.raw_handle()).cast_mut().cast();
                let cmd: *mut AnyObject = Retained::as_ptr(&commands).cast_mut().cast();
                self.server.publish_metal_texture(tex, cmd);
                commands.commit();
            }
        }
    }
}
