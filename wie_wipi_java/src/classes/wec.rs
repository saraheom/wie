use alloc::vec;

use java_class_proto::JavaMethodProto;
use java_constants::MethodAccessFlags;
use java_runtime::classes::java::lang::String;
use jvm::{Array, ClassInstanceRef, Jvm, Result as JvmResult};

use wie_jvm_support::{WieJavaClassProto, WieJvmContext};

// LGT/WIPI carrier extension used by some later titles.
//
// OZ imports this class as a platform-provided class with one direct/static
// method and no instance state. The real handset implementation launches an
// OEM/carrier application. WIE has no external carrier-app environment, so
// expose the ABI and report unsupported execution conservatively.
pub struct OEMAppExecutor;

impl OEMAppExecutor {
    pub fn as_proto() -> WieJavaClassProto {
        WieJavaClassProto {
            name: "wec/OEMAppExecutor",
            parent_class: Some("java/lang/Object"),
            interfaces: vec![],
            methods: vec![JavaMethodProto::new(
                "appExecutor",
                "(Ljava/lang/String;Ljava/lang/String;[[B)I",
                Self::app_executor,
                MethodAccessFlags::STATIC,
            )],
            fields: vec![],
            access_flags: Default::default(),
        }
    }

    async fn app_executor(
        _: &Jvm,
        _: &mut WieJvmContext,
        application: ClassInstanceRef<String>,
        command: ClassInstanceRef<String>,
        parameters: ClassInstanceRef<Array<Array<i8>>>,
    ) -> JvmResult<i32> {
        tracing::warn!(
            "[PHASE8_57_WEC_OEM_APP_EXECUTOR] application={application:?} command={command:?} parameters={parameters:?} result=-1 unsupported_external_carrier_app"
        );

        // Negative result: no OEM/carrier application was launched. Do not
        // report a false success for an external service that WIE cannot run.
        Ok(-1)
    }
}
