#![no_std]
extern crate alloc;
// Phase 8.26: host-side LZMA decoder uses std::io traits on supported targets.
extern crate std;

mod adf;
mod dump;
mod emulator;
mod runtime;

pub use dump::dump_image;
pub use emulator::KtfEmulator;
