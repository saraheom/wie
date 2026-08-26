use core::pin::Pin;

use alloc::{borrow::ToOwned, boxed::Box, collections::BTreeMap, format, string::String, vec, vec::Vec};

use jvm::{ClassInstance, Result as JvmResult, runtime::JavaLangString};

use wie_backend::{Emulator, Event, KeyCode, Options, Platform, System, TaskRunner};
use wie_core_arm::{Allocator, ArmCore};
use wie_jvm_support::JvmSupport;
use wie_util::{ByteRead, ByteWrite, Result, WieError};

use crate::{
    adf::{KtfAdf, find_client_bin},
    runtime::KtfJvmSupport,
};

pub const IMAGE_BASE: u32 = 0x100000;

struct KtfTaskRunner {
    core: ArmCore,
}

#[async_trait::async_trait]
impl TaskRunner for KtfTaskRunner {
    async fn run(&self, future: Pin<Box<dyn Future<Output = Result<()>> + Send>>) -> Result<()> {
        self.core.run_in_thread(async move || future.await)?.await
    }
}

pub struct KtfEmulator {
    core: ArmCore,
    system: System,
    inotia1_exp_diag_available: bool,
}

impl KtfEmulator {
    pub fn from_archive(platform: Box<dyn Platform>, files: BTreeMap<String, Vec<u8>>, options: Options) -> Result<Self> {
        let adf = files
            .get("__adf__")
            .ok_or_else(|| WieError::FatalError("Missing __adf__ in KTF archive".into()))?;
        let adf = KtfAdf::parse(adf);

        tracing::info!("Loading app {}, pid {}, mclass {}", adf.aid, adf.pid, adf.mclass);
        if let Some((width, height)) = adf.display_size
            && let Err(error) = platform.screen().resize(width, height)
        {
            tracing::warn!("Ignoring unsupported display size {width}x{height}: {error}");
        }

        let jar_filename = format!("{}.jar", adf.aid);

        Self::load(platform, &jar_filename, &adf.pid, &adf.aid, Some(adf.mclass), &files, options)
    }

    pub fn from_jar(
        platform: Box<dyn Platform>,
        jar_filename: &str,
        jar: Vec<u8>,
        pid: &str,
        aid: &str,
        main_class_name: Option<String>,
        options: Options,
    ) -> Result<Self> {
        let files = [(jar_filename.to_owned(), jar)].into_iter().collect();

        Self::load(platform, jar_filename, pid, aid, main_class_name, &files, options)
    }

    pub fn loadable_archive(files: &BTreeMap<String, Vec<u8>>) -> bool {
        files.contains_key("__adf__")
    }

    pub fn loadable_jar(jar: &[u8]) -> bool {
        find_client_bin(jar).is_ok()
    }

    fn load(
        platform: Box<dyn Platform>,
        jar_filename: &str,
        pid: &str,
        aid: &str,
        main_class_name: Option<String>,
        files: &BTreeMap<String, Vec<u8>>,
        mut options: Options,
    ) -> Result<Self> {
        let mut core = ArmCore::new(options.enable_gdbserver, options.profile.take())?;
        let inotia1_exp_diag_available = aid == "010100D3" && pid == "PD005362";
        if inotia1_exp_diag_available {
            // Phase 8.45: 8.44 proved the widened 16/32-bit watcher works, but
            // startup initialization consumed all 480 events before gameplay.
            // Keep the watcher DISARMED until the player explicitly presses the
            // Arm/Reset EXP Trace button beside the in-game diagnostics tools.
            core.set_inotia1_exp_diagnostics(false);
            tracing::info!(
                "[PHASE8_45_RUNTIME_SENTINEL] WIPI Player Phase 8.45 active; Inotia1 manual EXP-store/object diagnostic available (read-only)"
            );
            tracing::info!(
                "[PHASE8_45_INOTIA1_EXP_TRACE_AVAILABLE] watcher starts disarmed; press Arm/Reset EXP Trace immediately before combat to reset and begin 16/32-bit candidate capture"
            );
        }
        if aid == "010100D5" && pid == "PD007974" {
            // Phase 8.22 — latency-first Inotia 2 execution profile.
            //
            // Phase 8.21 raised this title from 4k to 16k guest instructions
            // per cooperative slice, but field testing showed no visible
            // improvement in menu animation, skills, or map transitions.
            // Restore the better-observed 4k balance while the interpreter
            // memory hot path is optimized separately. Other titles remain on
            // the default 1k budget.
            core.set_run_slice_instructions(4_000);
            core.set_thread_lifecycle_logging(false);
            // Phase 8.30 — narrow startup/main-menu native-loop probe.
            //
            // Phase 8.29 proves the corrected LZMA and RGB565 host paths are
            // active, yet roughly eight seconds remain between the final static
            // resource writeback and the first full-screen RGB batch. Gameplay
            // is otherwise stable, so do not make another speculative scheduler
            // or graphics change. Lower only the one-shot NATIVE_LOOP threshold
            // from the production 16,384 chunks (~65.5M instructions at 4k) to
            // 2,048 chunks (~8.2M). Each run_function can log at most once at
            // this equality threshold, giving us the exact PC/LR for the black
            // startup and laggy main-menu work without restoring the old frame
            // stall profiler or changing execution semantics.
            core.set_native_loop_trace_chunks(2_048);
            tracing::info!(
                "[PHASE8_30_INOTIA2_STARTUP_MENU_PROBE] native run slice=4000; NATIVE_LOOP threshold=2048 chunks (~8.2M instructions); frame stall profiler remains disabled"
            );
        }
        let system = System::new(platform, pid, aid, KtfTaskRunner { core: core.clone() });

        for (path, data) in files {
            let path = path.trim_start_matches("P/");
            system.filesystem().add_virtual(path, data.clone());
        }

        Allocator::init(&mut core)?;

        let mut core_clone = core.clone();
        let mut system_clone = system.clone();
        let jar_filename_clone = jar_filename.to_owned();

        system.spawn(async move || Self::start(&mut core_clone, &mut system_clone, jar_filename_clone, main_class_name).await);

        Ok(Self { core, system, inotia1_exp_diag_available })
    }

    #[tracing::instrument(name = "start", skip_all)]
    async fn start(core: &mut ArmCore, system: &mut System, jar_filename: String, main_class_name: Option<String>) -> Result<()> {
        let (jvm, class_loader) = KtfJvmSupport::init(core, system, Some(&jar_filename)).await?;

        let main_class_name = if let Some(x) = main_class_name {
            x
        } else {
            return Err(WieError::FatalError("Main class not found".into()));
        };

        let main_class_name = main_class_name.replace('.', "/");

        let main_class_name_java = JavaLangString::from_rust_string(&jvm, &main_class_name).await.unwrap();
        let _main_class: Box<dyn ClassInstance> = jvm
            .invoke_virtual(
                &class_loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                (main_class_name_java.clone(),),
            )
            .await
            .unwrap();

        let mut args_array = jvm.instantiate_array("Ljava/lang/String;", 1).await.unwrap();
        jvm.store_array(&mut args_array, 0, vec![main_class_name_java]).await.unwrap();
        let result: JvmResult<()> = jvm
            .invoke_static("org/kwis/msp/lcdui/Main", "main", "([Ljava/lang/String;)V", (args_array,))
            .await;

        if let Err(x) = result {
            return Err(JvmSupport::to_wie_err(&jvm, x).await);
        }

        Ok(())
    }
}

impl KtfEmulator {
    // Phase 8.40 — emergency-only pre-purchase CLEAR recovery.
    //
    // The Phase 8.38 field log proves the missing-prayer flow is first seen in
    // native state 14, then the nested cash UI reconnects after changing that
    // outer state to 6.  Clearing the resulting cash error does not emit
    // command 123, so a protocol-only recovery never gets a chance to run.
    // This hook executes only for a CLEAR keydown while the rare state-14
    // origin latch is active *before a purchase succeeds*. Phase 8.40 retires
    // that latch as soon as the emergency command-31 success frame is fully
    // delivered, so CLEAR after a successful 부활의 기도문 purchase is left
    // entirely to the original cash/death UI. Pre-purchase cancellation still
    // writes the exact destination used by the native state-14 CLEAR handler
    // (state 11, selection 0) and forwards CLEAR normally. There is no work on
    // ordinary movement keys beyond one atomic branch, preserving the Phase
    // 8.37 performance baseline.
    fn phase8_40_restore_party_wipe_prompt_on_clear(&mut self) {
        if !wie_wipi_c::api::net::phase8_40_inotia1_emergency_prayer_cash_active() {
            return;
        }

        const INOTIA1_GOT_BASE: u32 = 0x0016_883c;
        const STATE_GOT_OFFSET: u32 = 0x25c;
        const SELECTION_GOT_OFFSET: u32 = 0x5f8;

        let read_u32 = |core: &ArmCore, address: u32| -> Option<u32> {
            let mut bytes = [0u8; 4];
            core.read_bytes(address, &mut bytes).ok()?;
            Some(u32::from_le_bytes(bytes))
        };

        let Some(state_ptr) = read_u32(&self.core, INOTIA1_GOT_BASE + STATE_GOT_OFFSET) else {
            tracing::warn!("[PHASE8_40_INOTIA1_WIPE_CLEAR_EVENT_RECOVERY] state pointer unavailable; latch retained");
            return;
        };
        let Some(selection_ptr) = read_u32(&self.core, INOTIA1_GOT_BASE + SELECTION_GOT_OFFSET) else {
            tracing::warn!("[PHASE8_40_INOTIA1_WIPE_CLEAR_EVENT_RECOVERY] selection pointer unavailable; latch retained");
            return;
        };
        let old_state = read_u32(&self.core, state_ptr).unwrap_or(u32::MAX);
        let old_selection = read_u32(&self.core, selection_ptr).unwrap_or(u32::MAX);

        let state_ok = self.core.write_bytes(state_ptr, &11u32.to_le_bytes()).is_ok();
        let selection_ok = self.core.write_bytes(selection_ptr, &0u32.to_le_bytes()).is_ok();
        if state_ok && selection_ok {
            wie_wipi_c::api::net::phase8_40_clear_inotia1_emergency_prayer_cash_latch();
        }
        tracing::info!(
            "[PHASE8_40_INOTIA1_WIPE_CLEAR_EVENT_RECOVERY] CLEAR while emergency latch active: state {old_state}->11 selection {old_selection}->0 state_write={state_ok} selection_write={selection_ok}; CLEAR forwarded normally"
        );
    }
}

impl Emulator for KtfEmulator {
    fn handle_event(&mut self, event: Event) {
        if matches!(&event, Event::Keydown(KeyCode::CLEAR)) {
            self.phase8_40_restore_party_wipe_prompt_on_clear();
        }
        self.system.event_queue().push(event)
    }

    fn set_inotia1_exp_trace_armed(&mut self, armed: bool) -> bool {
        if !self.inotia1_exp_diag_available {
            return false;
        }
        self.core.set_inotia1_exp_diagnostics(armed);
        if armed {
            tracing::info!(
                "[PHASE8_45_INOTIA1_EXP_TRACE_MANUALLY_ARMED] candidate counter/reset complete; 16/32-bit watcher active now; repeated address+callsite writes capped; event_limit=600"
            );
        } else {
            tracing::info!("[PHASE8_45_INOTIA1_EXP_TRACE_DISARMED] watcher disabled");
        }
        true
    }

    fn tick(&mut self) -> Result<()> {
        self.system.tick().map_err(|x| {
            let reg_stack = self.core.dump_reg_stack(IMAGE_BASE);
            match x {
                WieError::FatalError(msg) => WieError::FatalError(format!("{msg}\n{reg_stack}")),
                _ => WieError::FatalError(format!("{x}\n{reg_stack}")),
            }
        })
    }
}
