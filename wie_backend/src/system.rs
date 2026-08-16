mod audio;
mod event_queue;
mod file_system;

use alloc::{borrow::ToOwned, boxed::Box, string::String, sync::Arc};

use spin::{RwLock, RwLockWriteGuard};

use wie_util::Result;

use crate::{
    AsyncCallable,
    executor::Executor,
    platform::Platform,
    task::{SleepFuture, YieldFuture},
    task_runner::TaskRunner,
};

use self::{audio::Audio, event_queue::EventQueue};

pub use self::{
    event_queue::{Event, KeyCode},
    file_system::FilesystemOverlay,
};

#[derive(Clone)]
pub struct System {
    pid: String,
    aid: String,
    legacy_phone_number: String,
    legacy_phone_model: String,
    legacy_esn: String,
    executor: Executor,
    platform: Arc<Box<dyn Platform>>,
    filesystem: FilesystemOverlay,
    event_queue: Arc<RwLock<EventQueue>>,
    audio: Arc<RwLock<Audio>>,
    task_runner: Arc<dyn TaskRunner>,
}

impl System {
    pub fn new<T>(platform: Box<dyn Platform>, pid: &str, aid: &str, task_runner: T) -> Self
    where
        T: TaskRunner + 'static,
    {
        let audio_sink = platform.audio_sink();
        let platform = Arc::new(platform);

        Self {
            pid: pid.to_owned(),
            aid: aid.to_owned(), // TODO create metadata dictionary or something
            legacy_phone_number: String::new(),
            legacy_phone_model: "Emulator".into(),
            legacy_esn: String::new(),
            executor: Executor::new(),
            filesystem: FilesystemOverlay::new(platform.clone(), aid),
            platform,
            event_queue: Arc::new(RwLock::new(EventQueue::new())),
            audio: Arc::new(RwLock::new(Audio::new(audio_sink))),
            task_runner: Arc::new(task_runner),
        }
    }

    pub fn tick(&mut self) -> Result<()> {
        let platform = self.platform.clone();
        self.executor.tick(move || platform.now())
    }

    pub fn spawn<C>(&self, callable: C)
    where
        C: AsyncCallable<Result<()>> + 'static + Send,
    {
        let runner_clone = self.task_runner.clone();
        self.executor.spawn(async move || runner_clone.run(Box::pin(callable.call())).await);
    }

    pub fn sleep(&self, timeout: u64) -> SleepFuture {
        SleepFuture::new(timeout, &self.executor)
    }

    pub fn current_task_id(&self) -> u64 {
        self.executor.current_task_id()
    }

    pub fn yield_now(&self) -> YieldFuture {
        YieldFuture::new()
    }

    /// Unified filesystem view. Reads consult the persistent platform
    /// backend first and fall back to the in-memory virtual layer loaded
    /// from archives; writes always hit the platform backend.
    pub fn filesystem(&self) -> &FilesystemOverlay {
        &self.filesystem
    }

    pub fn pid(&self) -> &str {
        &self.pid
    }

    pub fn aid(&self) -> &str {
        &self.aid
    }

    /// Configure the identity of the legacy handset represented by this
    /// emulator session. LGT OMA archives often preserve the original CTN
    /// (phone number) and device_id in app_info/DDurl. Commercial WIPI titles
    /// used those values for local first-run/license checks.
    pub fn set_legacy_device_identity(&mut self, phone_number: Option<&str>, phone_model: Option<&str>) {
        if let Some(phone_number) = phone_number.filter(|value| !value.is_empty()) {
            self.legacy_phone_number = phone_number.to_owned();
            // There is no ESN field in the archived OMA metadata. A stable,
            // numeric per-download identifier is more compatible than returning
            // M_E_INVALID; use the archived CTN as the deterministic fallback.
            self.legacy_esn = phone_number.to_owned();
        }
        if let Some(phone_model) = phone_model.filter(|value| !value.is_empty()) {
            self.legacy_phone_model = phone_model.to_owned();
        }
    }

    pub fn legacy_phone_number(&self) -> &str {
        &self.legacy_phone_number
    }

    pub fn legacy_phone_model(&self) -> &str {
        &self.legacy_phone_model
    }

    pub fn legacy_esn(&self) -> &str {
        &self.legacy_esn
    }

    pub fn platform(&self) -> &dyn Platform {
        self.platform.as_ref().as_ref()
    }

    pub fn audio(&self) -> RwLockWriteGuard<'_, Audio> {
        self.audio.as_ref().write()
    }

    pub fn event_queue(&self) -> RwLockWriteGuard<'_, EventQueue> {
        self.event_queue.write()
    }
}
