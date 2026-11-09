//! The openVJ composition model.
//!
//! Pure data and playback logic with no GPU, UI or decoding dependencies,
//! so it can be unit-tested headless and reused by any front end.

mod composition;
mod effect;
pub mod effect_folders;
mod generator;
pub mod isf;
mod midi;
mod modulation;
mod output;
mod playback;
mod tempo;

pub use composition::*;
pub use effect::*;
pub use generator::*;
pub use midi::*;
pub use modulation::*;
pub use output::*;
pub use playback::*;
pub use tempo::*;
