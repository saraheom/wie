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
}

impl JavaSvcFunctions {
    pub fn new() -> Self {
        Self {
            functions: Arc::new(Mutex::new(BTreeMap::new())),
            metadata: Arc::new(Mutex::new(BTreeMap::new())),
            hot_loop: Arc::new(Mutex::new((0, 0))),
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
    let hot_count = functions.note_hot_loop(id.0);
    if hot_count == 100 || hot_count == 1000 || hot_count % 10000 == 0 {
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
    tracing::info!(
        "[PHASE8_64_OZ_JAVA_CALL_ENTRY] id={:#010x} class={} method={} descriptor={} lr={:#010x} r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x}",
        id.0, class_name, method_name, method_descriptor, lr, r0, r1, r2, r3
    );

    // Phase 8.67: the Phase 8.66 return-null experiment is intentionally
    // removed. OZ binary.mod is now bootstrapped directly from the mounted JAR,
    // preserving normal URLClassLoader semantics for later resource lookups.
    let call_result = function.call(core).await;
    let return_r0 = core.read_param(0).unwrap_or(0);
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
    tracing::info!(
        "[PHASE8_64_OZ_JAVA_CALL_RETURN] id={:#010x} class={} method={} descriptor={} ok={} r0={:#010x}",
        id.0, class_name, method_name, method_descriptor, call_result.is_ok(), return_r0
    );

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
