use alloc::{boxed::Box, collections::BTreeMap, format, string::String, sync::Arc};

use spin::Mutex;
use wipi_types::lgt::java::{
    LgtJavaClass as RawJavaClass, LgtJavaClassDescriptor as RawJavaClassDescriptor, LgtJavaClassInstance as RawJavaClassInstance, LgtJavaClassMethod as RawJavaMethod,
};

use wie_core_arm::{ArmCore, JumpTo, RegisteredFunction, SvcId};
use wie_util::{Result, WieError, read_generic, read_null_terminated_string_bytes};

use crate::runtime::{SVC_CATEGORY_JAVA, SVC_CATEGORY_MISSING_JAVA_VTABLE_ENTRY};

mod abi;
pub mod classes;
mod exception;
mod interface;
mod jvm_support;

pub use interface::{get_java_interface_method, register_java_system_svc_handler};
pub use jvm_support::LgtJvmSupport;

#[derive(Clone)]
pub struct JavaSvcFunctions {
    functions: Arc<Mutex<BTreeMap<u32, Arc<Box<dyn RegisteredFunction>>>>>,
    metadata: Arc<Mutex<BTreeMap<u32, (String, String, String)>>>,
    hot_loop: Arc<Mutex<(u32, u32)>>,
    oz_exception_loop_hits: Arc<Mutex<u32>>,
}

impl JavaSvcFunctions {
    pub fn new() -> Self {
        Self {
            functions: Arc::new(Mutex::new(BTreeMap::new())),
            metadata: Arc::new(Mutex::new(BTreeMap::new())),
            hot_loop: Arc::new(Mutex::new((0, 0))),
            oz_exception_loop_hits: Arc::new(Mutex::new(0)),
        }
    }

    pub fn insert(&self, id: u32, function: Arc<Box<dyn RegisteredFunction>>, metadata: Option<(String, String, String)>) {
        self.functions.lock().insert(id, function);
        if let Some(metadata) = metadata {
            self.metadata.lock().insert(id, metadata);
        }
    }

    fn get_function(&self, id: u32) -> Option<Arc<Box<dyn RegisteredFunction>>> {
        self.functions.lock().get(&id).cloned()
    }

    fn metadata(&self, id: u32) -> Option<(String, String, String)> {
        self.metadata.lock().get(&id).cloned()
    }

    fn note_hot_loop(&self, id: u32) -> u32 {
        let mut state = self.hot_loop.lock();
        if state.0 == id {
            state.1 = state.1.saturating_add(1);
        } else {
            *state = (id, 1);
        }
        state.1
    }

    fn note_oz_exception_loop(&self) -> u32 {
        let mut hits = self.oz_exception_loop_hits.lock();
        *hits = hits.saturating_add(1);
        *hits
    }
}

fn phase8_83_thumb_window(core: &ArmCore, start: u32, end: u32) -> String {
    let mut out = String::new();
    let mut address = start;
    while address < end {
        if !out.is_empty() {
            out.push(' ');
        }
        match read_generic::<u16, _>(core, address) {
            Ok(value) => out.push_str(&format!("{address:#08x}:{value:04x}")),
            Err(_) => out.push_str(&format!("{address:#08x}:????")),
        }
        address = address.wrapping_add(2);
    }
    out
}

fn phase8_83_stack_words(core: &ArmCore, sp: u32) -> String {
    let mut out = String::new();
    for index in 0..12u32 {
        if index != 0 {
            out.push(' ');
        }
        let address = sp.wrapping_add(index * 4);
        match read_generic::<u32, _>(core, address) {
            Ok(value) => out.push_str(&format!("{address:#010x}:{value:#010x}")),
            Err(_) => out.push_str(&format!("{address:#010x}:<unreadable>")),
        }
    }
    out
}

fn phase8_71_decode_java_string(core: &ArmCore, ptr: u32) -> String {
    if ptr == 0 {
        return String::from("<null>");
    }
    let data_ptr = match read_generic::<u32, _>(core, ptr.wrapping_add(16)) {
        Ok(v) => v,
        Err(_) => return format!("<unreadable-string@{:#010x}>", ptr),
    };
    let count = match read_generic::<u32, _>(core, ptr.wrapping_add(24)) {
        Ok(v) => v.min(512),
        Err(_) => return format!("<unreadable-length@{:#010x}>", ptr),
    };
    let mut units = alloc::vec::Vec::new();
    for i in 0..count {
        match read_generic::<u16, _>(core, data_ptr.wrapping_add(i * 2)) {
            Ok(v) => units.push(v),
            Err(_) => return format!("<bad-utf16@{:#010x}>", data_ptr),
        }
    }
    String::from_utf16(&units).unwrap_or_else(|_| format!("<invalid-utf16@{:#010x}>", data_ptr))
}

fn phase8_65_probe_words(core: &ArmCore, ptr: u32) -> String {
    if ptr == 0 {
        return String::from("<null>");
    }
    let mut out = String::new();
    for index in 0..8u32 {
        let address = ptr.wrapping_add(index * 4);
        if index != 0 {
            out.push(' ');
        }
        match read_generic::<u32, _>(core, address) {
            Ok(value) => out.push_str(&format!("{:#010x}", value)),
            Err(_) => out.push_str("<unreadable>"),
        }
    }
    out
}



fn phase8_86_decode_java_string(core: &ArmCore, ptr: u32) -> String {
    if ptr == 0 {
        return String::from("<null>");
    }
    let instance: RawJavaClassInstance = match read_generic(core, ptr) {
        Ok(v) => v,
        Err(_) => return format!("<unreadable-string-instance@{ptr:#010x}>"),
    };
    let fields = instance.ptr_fields;
    let value_ptr: u32 = match read_generic(core, fields) {
        Ok(v) => v,
        Err(_) => return format!("<unreadable-string-fields@{fields:#010x}>"),
    };
    if value_ptr == 0 {
        return String::new();
    }
    let array_instance: RawJavaClassInstance = match read_generic(core, value_ptr) {
        Ok(v) => v,
        Err(_) => return format!("<bad-char-array@{value_ptr:#010x}>"),
    };
    let array_fields = array_instance.ptr_fields;
    let array_len: u32 = match read_generic(core, array_fields) {
        Ok(v) => v.min(4096),
        Err(_) => return format!("<bad-char-array-length@{array_fields:#010x}>"),
    };
    let f1: u32 = read_generic(core, fields.wrapping_add(4)).unwrap_or(0);
    let f2: u32 = read_generic(core, fields.wrapping_add(8)).unwrap_or(array_len);

    // Classic CLDC/Java String layouts use value:[C plus offset/count.
    // Accept both observed field orderings and choose the first sane range.
    let candidates = [(f1, f2), (f2, f1), (0, array_len)];
    for (offset, count) in candidates {
        if offset <= array_len && count <= array_len.saturating_sub(offset) && count <= 2048 {
            let mut units = alloc::vec::Vec::with_capacity(count as usize);
            let mut ok = true;
            for i in 0..count {
                let address = array_fields.wrapping_add(4).wrapping_add((offset + i) * 2);
                match read_generic::<u16, _>(core, address) {
                    Ok(v) => units.push(v),
                    Err(_) => { ok = false; break; }
                }
            }
            if ok {
                if let Ok(text) = String::from_utf16(&units) {
                    // Reject obviously implausible raw-memory decodes when a
                    // later candidate may represent the real offset/count.
                    let printable = text.chars().filter(|c| !c.is_control() || matches!(*c, '\n' | '\r' | '\t')).count();
                    if text.is_empty() || printable * 2 >= text.chars().count().max(1) {
                        return text;
                    }
                }
            }
        }
    }
    format!("<undecodable-java-string@{ptr:#010x} fields={fields:#010x} value={value_ptr:#010x} len={array_len} f1={f1} f2={f2}>")
}

fn phase8_86_exception_message(core: &ArmCore, ptr_exception: u32) -> String {
    let instance: RawJavaClassInstance = match read_generic(core, ptr_exception) {
        Ok(v) => v,
        Err(_) => return String::from("<unreadable-exception>"),
    };
    // Throwable.detailMessage is the first inherited instance field in the
    // RustJava layout used by LGT. Phase 8.85 confirmed this exact pointer is
    // also passed to WieError(String).
    let message_ptr: u32 = read_generic(core, instance.ptr_fields).unwrap_or(0);
    phase8_86_decode_java_string(core, message_ptr)
}

fn phase8_85_exception_class_name(core: &ArmCore, ptr_exception: u32) -> String {
    let instance: RawJavaClassInstance = match read_generic(core, ptr_exception) {
        Ok(v) => v,
        Err(_) => return format!("<unreadable-exception@{ptr_exception:#010x}>"),
    };
    let ptr_class: u32 = match read_generic(core, instance.ptr_dispatch_table) {
        Ok(v) => v,
        Err(_) => return format!("<bad-vtable@{:#010x}>", instance.ptr_dispatch_table),
    };
    let class: RawJavaClass = match read_generic(core, ptr_class) {
        Ok(v) => v,
        Err(_) => return format!("<bad-class@{ptr_class:#010x}>"),
    };
    let descriptor: RawJavaClassDescriptor = match read_generic(core, class.ptr_descriptor) {
        Ok(v) => v,
        Err(_) => return format!("<bad-descriptor@{:#010x}>", class.ptr_descriptor),
    };
    String::from_utf8(read_null_terminated_string_bytes(core, descriptor.ptr_name).unwrap_or_default())
        .unwrap_or_else(|_| format!("<invalid-class-name@{:#010x}>", descriptor.ptr_name))
}

fn phase8_85_exception_field_probe(core: &ArmCore, ptr_exception: u32) -> String {
    let instance: RawJavaClassInstance = match read_generic(core, ptr_exception) {
        Ok(v) => v,
        Err(_) => return String::from("<unreadable-instance>"),
    };
    let mut out = String::new();
    for index in 0..16u32 {
        let address = instance.ptr_fields.wrapping_add(index * 4);
        let value: u32 = match read_generic(core, address) {
            Ok(v) => v,
            Err(_) => break,
        };
        if index != 0 {
            out.push(' ');
        }
        out.push_str(&format!("f{index}={value:#010x}"));
        if value != 0 {
            let decoded = phase8_71_decode_java_string(core, value);
            if !decoded.starts_with('<') && !decoded.is_empty() {
                let safe = decoded.chars().take(160).collect::<String>();
                out.push_str(&format!("({safe:?})"));
            }
        }
    }
    out
}

fn phase8_84_recent_guest_pc_trace(core: &mut ArmCore) -> String {
    let history = core.guest_pc_history();
    let mut out = String::new();
    for (index, pc) in history.iter().enumerate() {
        if index != 0 {
            out.push_str(" ");
        }
        let aligned = *pc & !1;
        let opcode: Result<u16> = read_generic(core, aligned);
        match opcode {
            Ok(value) => out.push_str(&format!("{aligned:#010x}:{value:#06x}")),
            Err(_) => out.push_str(&format!("{aligned:#010x}:????")),
        }
    }
    out
}

async fn handle_java_svc(core: &mut ArmCore, functions: &mut JavaSvcFunctions, id: SvcId) -> Result<JumpTo> {
    let (_, lr) = core.read_pc_lr()?;
    let function = functions
        .get_function(id.0)
        .ok_or_else(|| WieError::FatalError(alloc::format!("Unknown LGT Java SVC id {:#x}", id.0)))?;

    // Phase 8.64: resolve the dynamic LGT Java dispatcher id (r12/ptr_method)
    // into class/method/descriptor so an unmatched call can be identified directly.
    let (class_name, method_name, method_descriptor) = functions.metadata(id.0).unwrap_or_else(|| {
        (|| -> Result<(String, String, String)> {
            let method: RawJavaMethod = read_generic(core, id.0)?;
            let method_name = String::from_utf8(read_null_terminated_string_bytes(core, method.ptr_name)?)
                .unwrap_or_else(|_| alloc::format!("<invalid-name@{:#x}>", method.ptr_name));
            let method_descriptor = String::from_utf8(read_null_terminated_string_bytes(core, method.ptr_descriptor)?)
                .unwrap_or_else(|_| alloc::format!("<invalid-desc@{:#x}>", method.ptr_descriptor));
            let class: RawJavaClass = read_generic(core, method.ptr_class)?;
            let descriptor: RawJavaClassDescriptor = read_generic(core, class.ptr_descriptor)?;
            let class_name = String::from_utf8(read_null_terminated_string_bytes(core, descriptor.ptr_name)?)
                .unwrap_or_else(|_| alloc::format!("<invalid-class@{:#x}>", descriptor.ptr_name));
            Ok((class_name, method_name, method_descriptor))
        })().unwrap_or_else(|_| (String::from("<unknown-class>"), String::from("<unknown-method>"), String::from("<unknown-desc>")))
    });
    let r0 = core.read_param(0).unwrap_or(0);
    let r1 = core.read_param(1).unwrap_or(0);
    let r2 = core.read_param(2).unwrap_or(0);
    let r3 = core.read_param(3).unwrap_or(0);

    // Phase 8.83 — 8.82 resolved OZ's apparent freeze to a tight guest loop
    // repeatedly requesting java/lang/Exception.<get-initialized-class> from
    // caller LR 0x17b70. Capture the complete Thumb basic-block window and
    // guest register/stack state at that exact boundary. The full code window
    // is emitted once; compact register snapshots are repeated at sparse
    // milestones so a changing loop condition is visible without flooding the
    // persistent log.
    let mut phase8_83_exception_loop_hit = 0u32;
    if class_name == "java/lang/Exception"
        && method_name == "<get-initialized-class>"
        && (0x0001_7b20..=0x0001_7ba0).contains(&lr)
    {
        let hit = functions.note_oz_exception_loop();
        phase8_83_exception_loop_hit = hit;
        let regs = core.save_context();
        if hit == 1 {
            tracing::warn!(
                "[PHASE8_84_OZ_FIRST_LOOP_GUEST_PC_TRACE] hit={hit} caller_lr={lr:#010x} recent_pc_opcode=[{}]",
                phase8_84_recent_guest_pc_trace(core)
            );
            tracing::warn!(
                "[PHASE8_84_OZ_FIRST_LOOP_REGS] hit={hit} trampoline_pc={:#010x} caller_lr={lr:#010x} sp={:#010x} cpsr={:#010x} r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x} r4={:#010x} r5={:#010x} r6={:#010x} r7={:#010x} stack=[{}]",
                regs.pc, regs.sp, regs.cpsr, regs.r0, regs.r1, regs.r2, regs.r3, regs.r4, regs.r5, regs.r6, regs.r7,
                phase8_83_stack_words(core, regs.sp)
            );
            tracing::warn!(
                "[PHASE8_84_OZ_CALLSITE_CODE_WINDOW] caller_lr={lr:#010x} range=0x00017b20-0x00017ba0 thumb=[{}]",
                phase8_83_thumb_window(core, 0x0001_7b20, 0x0001_7ba0)
            );
        } else if matches!(hit, 10 | 100 | 1000) {
            tracing::warn!(
                "[PHASE8_84_OZ_EXCEPTION_LOOP_SANITY] hit={hit} caller_lr={lr:#010x} pc={:#010x} sp={:#010x} r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x} r4={:#010x} r5={:#010x} r6={:#010x} r7={:#010x}",
                regs.pc, regs.sp, regs.r0, regs.r1, regs.r2, regs.r3, regs.r4, regs.r5, regs.r6, regs.r7
            );
        }
    }

    let hot_count = functions.note_hot_loop(id.0);
    if (phase8_83_exception_loop_hit == 0 && (hot_count == 100 || hot_count == 1000 || hot_count % 10000 == 0))
        || (phase8_83_exception_loop_hit != 0 && matches!(hot_count, 100 | 1000))
    {
        tracing::warn!(
            "[PHASE8_82_OZ_JAVA_HOT_LOOP] id={:#010x} class={} method={} descriptor={} consecutive_calls={} lr={:#010x} r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x}",
            id.0, class_name, method_name, method_descriptor, hot_count, lr, r0, r1, r2, r3
        );
    }
    if class_name == "java/net/URLClassLoader" && method_name == "findResource" {
        tracing::info!(
            "[PHASE8_71_OZ_FIND_RESOURCE_ENTRY] id={:#010x} loader={:#010x} name_object={:#010x} name={:?} name_words=[{}]",
            id.0, r0, r1, phase8_71_decode_java_string(core, r1), phase8_65_probe_words(core, r1)
        );
    }
    if class_name == "java/net/URL" && method_name == "getFile" {
        tracing::info!(
            "[PHASE8_65_OZ_URL_GET_FILE_ENTRY] id={:#010x} url_object={:#010x} url_words=[{}]",
            id.0, r0, phase8_65_probe_words(core, r0)
        );
    }
    // Once the OZ exception loop has been positively identified, suppress its
    // per-iteration generic Java entry records. Keeping only the first three
    // plus sparse Phase 8.83 milestones prevents the loop from evicting the
    // pre-failure context from the per-game persistent log/native ring.
    if phase8_83_exception_loop_hit == 0 || phase8_83_exception_loop_hit <= 3 {
        tracing::info!(
            "[PHASE8_64_OZ_JAVA_CALL_ENTRY] id={:#010x} class={} method={} descriptor={} lr={:#010x} r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x}",
            id.0, class_name, method_name, method_descriptor, lr, r0, r1, r2, r3
        );
    }

    // Phase 8.67: the Phase 8.66 return-null experiment is intentionally
    // removed. OZ binary.mod is now bootstrapped directly from the mounted JAR,
    // preserving normal URLClassLoader semantics for later resource lookups.
    let call_result = function.call(core).await;
    let return_r0 = core.read_param(0).unwrap_or(0);

    // Phase 8.85: 8.84 proved that OZ does not deadlock in the Exception
    // class getter itself. The guest's startApp() returns a JavaException and
    // only then falls into the 0x17b70 retry loop. Decode that thrown object
    // immediately, while its class/message fields are still intact, so one
    // test run identifies the real startup blocker rather than the secondary
    // exception-handler loop.
    if method_name == "startApp" {
        if let Err(WieError::JavaException(ptr_exception)) = &call_result {
            tracing::error!(
                "[PHASE8_85_OZ_STARTAPP_EXCEPTION] method_id={:#010x} exception={:#010x} class={} fields=[{}] lr={:#010x}",
                id.0, *ptr_exception, phase8_85_exception_class_name(core, *ptr_exception),
                phase8_85_exception_field_probe(core, *ptr_exception), lr
            );
            tracing::error!(
                "[PHASE8_86_OZ_STARTAPP_EXCEPTION_DECODED] method_id={:#010x} exception={:#010x} class={} message={:?} fields=[{}] lr={:#010x}",
                id.0, *ptr_exception, phase8_85_exception_class_name(core, *ptr_exception),
                phase8_86_exception_message(core, *ptr_exception),
                phase8_85_exception_field_probe(core, *ptr_exception), lr
            );
        }
    }
    if class_name == "java/net/URL" && method_name == "getFile" {
        tracing::info!(
            "[PHASE8_71_OZ_URL_GET_FILE_RETURN] id={:#010x} ok={} result={:#010x} file={:?} result_words=[{}]",
            id.0, call_result.is_ok(), return_r0, phase8_71_decode_java_string(core, return_r0), phase8_65_probe_words(core, return_r0)
        );
    }
    if class_name == "java/net/URLClassLoader" && method_name == "findResource" {
        tracing::info!(
            "[PHASE8_65_OZ_FIND_RESOURCE_RETURN] id={:#010x} ok={} result={:#010x} result_words=[{}]",
            id.0, call_result.is_ok(), return_r0, phase8_65_probe_words(core, return_r0)
        );
    }
    if phase8_83_exception_loop_hit == 0 || phase8_83_exception_loop_hit <= 3 {
        tracing::info!(
            "[PHASE8_64_OZ_JAVA_CALL_RETURN] id={:#010x} class={} method={} descriptor={} ok={} r0={:#010x}",
            id.0, class_name, method_name, method_descriptor, call_result.is_ok(), return_r0
        );
    }

    match call_result {
        Ok(()) => Ok(JumpTo(lr)),
        Err(WieError::JavaException(ptr_exception)) => match exception::unwind(core, ptr_exception)? {
            Some(resume_address) => Ok(JumpTo(resume_address)),
            None => Err(WieError::JavaException(ptr_exception)),
        },
        Err(error) => Err(error),
    }
}

async fn handle_missing_java_vtable_entry(core: &mut ArmCore, _: &mut (), id: SvcId) -> Result<JumpTo> {
    let ptr_instance = core.read_param(0)?;
    let instance: RawJavaClassInstance = read_generic(core, ptr_instance)?;
    let ptr_class: u32 = read_generic(core, instance.ptr_dispatch_table)?;
    let class: RawJavaClass = read_generic(core, ptr_class)?;
    let descriptor: RawJavaClassDescriptor = read_generic(core, class.ptr_descriptor)?;
    let class_name = String::from_utf8(read_null_terminated_string_bytes(core, descriptor.ptr_name)?)
        .map_err(|error| WieError::FatalError(format!("Invalid LGT class name: {error}")))?;

    Err(WieError::Unimplemented(format!("{class_name} vtable index {}", id.0)))
}

pub fn register_java_svc_handler(core: &mut ArmCore, functions: &JavaSvcFunctions) -> Result<()> {
    core.register_svc_handler(SVC_CATEGORY_JAVA, handle_java_svc, functions)?;
    core.register_svc_handler(SVC_CATEGORY_MISSING_JAVA_VTABLE_ENTRY, handle_missing_java_vtable_entry, &())
}
