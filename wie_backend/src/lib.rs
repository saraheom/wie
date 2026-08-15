#![no_std]
extern crate alloc;

mod audio_sink;
pub mod canvas;
mod database;
mod executor;
mod platform;
mod screen;
mod system;
mod task;
mod task_runner;
mod time;

pub use self::{
    audio_sink::AudioSink,
    database::{Database, DatabaseRepository, RecordId},
    executor::{AsyncCallable, AsyncCallableResult},
    platform::{Filesystem, Platform},
    screen::Screen,
    system::{Event, FilesystemOverlay, KeyCode, System},
    task::YieldFuture,
    task_runner::{DefaultTaskRunner, TaskRunner},
    time::Instant,
};

use alloc::{
    boxed::Box,
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};

use wie_util::{Result, WieError};

pub trait Emulator {
    fn handle_event(&mut self, event: Event);
    fn tick(&mut self) -> Result<()>;
}

pub struct ProfileSample {
    /// Leaf-first call stack: [pc, lr, lr_prev, ...].
    pub stack: Vec<u32>,
    pub count: u64,
}

/// Called periodically during execution with a batch of samples that the
/// profiler accumulated since the previous flush. The callback also fires once
/// more when the runtime shuts down to drain anything still in the buffer.
pub type ProfileCallback = Box<dyn FnMut(Vec<ProfileSample>) + Send + Sync>;

pub struct Options {
    pub enable_gdbserver: bool,
    pub profile: Option<ProfileCallback>,
}

pub fn extract_zip(zip: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    extern crate std; // XXX

    use std::io::{Cursor, Read};
    use zip::ZipArchive;

    let mut archive = ZipArchive::new(Cursor::new(zip)).map_err(|x| WieError::FatalError(format!("Invalid zip archive: {x}")))?;

    let mut files = BTreeMap::new();
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|err| WieError::FatalError(format!("Failed to read zip entry {index}: {err}")))?;
        if !file.is_file() {
            continue;
        }

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|err| WieError::FatalError(format!("Failed to read zip entry {}: {err}", file.name())))?;

        // Old Korean WIPI dumps are often packaged as:
        //   Game Name/__adf__
        //   Game Name/0101....jar
        // rather than placing the WIPI files at the ZIP root.  WIE's carrier
        // detectors expect __adf__/appinfo files at the root, so normalize path
        // separators here and strip one common wrapper directory below.
        let name = file.name().replace('\\', "/").trim_start_matches('/').to_string();
        files.insert(name, data);
    }

    // If every file is under the same first directory and there are no root
    // files, strip exactly that one directory. This preserves legitimate nested
    // game resources (p/foo.dat, res/bar.png, etc.) while accepting phone-dump
    // ZIPs that were wrapped in a folder by the archival tool.
    let common_prefix = files.keys().next().and_then(|first| {
        let (prefix, _) = first.split_once('/')?;
        (!prefix.is_empty()
            && files.keys().all(|name| name.starts_with(&format!("{prefix}/"))))
            .then(|| format!("{prefix}/"))
    });

    if let Some(prefix) = common_prefix {
        tracing::info!("Normalizing WIPI archive wrapper directory: {prefix}");
        files = files
            .into_iter()
            .map(|(name, data)| (name.strip_prefix(&prefix).unwrap_or(&name).to_string(), data))
            .collect();
    }

    Ok(files)
}
