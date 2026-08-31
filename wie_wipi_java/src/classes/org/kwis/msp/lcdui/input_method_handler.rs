use alloc::vec;

use java_class_proto::{JavaFieldProto, JavaMethodProto};
use jvm::{ClassInstanceRef, Jvm, Result as JvmResult};

use wie_jvm_support::{WieJavaClassProto, WieJvmContext};

// class org.kwis.msp.lcdui.InputMethodHandler
pub struct InputMethodHandler;

impl InputMethodHandler {
    pub fn as_proto() -> WieJavaClassProto {
        WieJavaClassProto {
            name: "org/kwis/msp/lcdui/InputMethodHandler",
            parent_class: Some("java/lang/Object"),
            interfaces: vec![],
            methods: vec![
                JavaMethodProto::new("<init>", "(I)V", Self::init, Default::default()),
                JavaMethodProto::new("setCurrentMode", "(I)Z", Self::set_current_mode, Default::default()),
                JavaMethodProto::new("getCurrentMode", "()I", Self::get_current_mode, Default::default()),
            ],
            fields: vec![JavaFieldProto::new("currentMode", "I", Default::default())],
            access_flags: Default::default(),
        }
    }

    async fn init(jvm: &Jvm, _: &mut WieJvmContext, mut this: ClassInstanceRef<Self>, constraint: i32) -> JvmResult<()> {
        tracing::debug!("org.kwis.msp.lcdui.InputMethodHandler::<init>({this:?}, {constraint})");

        let _: () = jvm.invoke_special(&this, "java/lang/Object", "<init>", "()V", ()).await?;

        // WIPI exposes a stateful current input mode.  Constraint-specific mode
        // selection is not yet emulated, so start from the neutral/default mode
        // and preserve any later setCurrentMode() value exactly.
        jvm.put_field(&mut this, "currentMode", "I", 0i32).await?;
        tracing::info!("[PHASE8_59_WIPI_INPUT_METHOD_MODE] op=init constraint={constraint} mode=0");

        Ok(())
    }

    async fn set_current_mode(jvm: &Jvm, _: &mut WieJvmContext, mut this: ClassInstanceRef<Self>, mode: i32) -> JvmResult<bool> {
        jvm.put_field(&mut this, "currentMode", "I", mode).await?;
        tracing::info!("[PHASE8_59_WIPI_INPUT_METHOD_MODE] op=set mode={mode}");

        Ok(true)
    }

    async fn get_current_mode(jvm: &Jvm, _: &mut WieJvmContext, this: ClassInstanceRef<Self>) -> JvmResult<i32> {
        let mode: i32 = jvm.get_field(&this, "currentMode", "I").await?;
        tracing::info!("[PHASE8_59_WIPI_INPUT_METHOD_MODE] op=get mode={mode}");

        Ok(mode)
    }
}
