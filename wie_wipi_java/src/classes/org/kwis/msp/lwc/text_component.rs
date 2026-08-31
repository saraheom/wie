use alloc::vec;

use java_class_proto::{JavaFieldProto, JavaMethodProto};
use java_runtime::classes::java::lang::String;
use jvm::{ClassInstanceRef, Jvm, Result as JvmResult, runtime::JavaLangString};

use wie_jvm_support::{WieJavaClassProto, WieJvmContext};

// class org.kwis.msp.lwc.TextComponent
pub struct TextComponent;

impl TextComponent {
    pub fn as_proto() -> WieJavaClassProto {
        WieJavaClassProto {
            name: "org/kwis/msp/lwc/TextComponent",
            parent_class: Some("org/kwis/msp/lwc/Component"),
            interfaces: vec![],
            methods: vec![
                JavaMethodProto::new("<init>", "()V", Self::init, Default::default()),
                JavaMethodProto::new("setMaxLength", "(I)V", Self::set_max_length, Default::default()),
                JavaMethodProto::new("getMaxLength", "()I", Self::get_max_length, Default::default()),
                JavaMethodProto::new("getString", "()Ljava/lang/String;", Self::get_string, Default::default()),
                JavaMethodProto::new("setString", "(Ljava/lang/String;)V", Self::set_string, Default::default()),
            ],
            fields: vec![
                JavaFieldProto::new("m_cPos", "I", Default::default()),
                JavaFieldProto::new("imHandler", "Lorg/kwis/msp/lcdui/InputMethodHandler;", Default::default()),
                JavaFieldProto::new("maxLength", "I", Default::default()),
                JavaFieldProto::new("constraint", "I", Default::default()),
                JavaFieldProto::new("text", "Ljava/lang/String;", Default::default()),
            ],
            access_flags: Default::default(),
        }
    }

    async fn init(jvm: &Jvm, _: &mut WieJvmContext, mut this: ClassInstanceRef<TextComponent>) -> JvmResult<()> {
        tracing::debug!("org.kwis.msp.lwc.TextComponent::<init>({this:?})");

        let _: () = jvm.invoke_special(&this, "org/kwis/msp/lwc/Component", "<init>", "()V", ()).await?;

        // WIPI TextComponent defaults to unlimited input length (-1) and
        // CONSTRAINT_ANY (0). Keep actual text state instead of the historical
        // constant "temp" getString() stub so AOT-linked LGT titles can round-trip
        // constructor/setString data correctly.
        let empty: ClassInstanceRef<String> = JavaLangString::from_rust_string(jvm, "").await?.into();
        jvm.put_field(&mut this, "m_cPos", "I", 0i32).await?;
        jvm.put_field(&mut this, "maxLength", "I", -1i32).await?;
        jvm.put_field(&mut this, "constraint", "I", 0i32).await?;
        jvm.put_field(&mut this, "text", "Ljava/lang/String;", empty).await?;

        // TODO constraint-specific input behavior. 0 = CONSTRAINT_ANY.
        let im_handler = jvm.new_class("org/kwis/msp/lcdui/InputMethodHandler", "(I)V", (0,)).await?;
        jvm.put_field(&mut this, "imHandler", "Lorg/kwis/msp/lcdui/InputMethodHandler;", im_handler).await?;

        tracing::info!("[PHASE8_56_WIPI_TEXT_COMPONENT] op=init max_length=-1 constraint=0");
        Ok(())
    }

    async fn set_max_length(jvm: &Jvm, _: &mut WieJvmContext, mut this: ClassInstanceRef<TextComponent>, max_length: i32) -> JvmResult<()> {
        // WIPI documents -1 as the unlimited default. Preserve any value the
        // application requests; input-length enforcement can be added when text
        // editing itself is implemented more fully.
        jvm.put_field(&mut this, "maxLength", "I", max_length).await?;
        tracing::info!("[PHASE8_56_WIPI_TEXT_COMPONENT] op=set_max_length value={max_length}");
        Ok(())
    }

    async fn get_max_length(jvm: &Jvm, _: &mut WieJvmContext, this: ClassInstanceRef<TextComponent>) -> JvmResult<i32> {
        let max_length: i32 = jvm.get_field(&this, "maxLength", "I").await?;
        tracing::info!("[PHASE8_56_WIPI_TEXT_COMPONENT] op=get_max_length value={max_length}");
        Ok(max_length)
    }

    async fn get_string(jvm: &Jvm, _: &mut WieJvmContext, this: ClassInstanceRef<TextComponent>) -> JvmResult<ClassInstanceRef<String>> {
        let text: ClassInstanceRef<String> = jvm.get_field(&this, "text", "Ljava/lang/String;").await?;
        tracing::debug!("org.kwis.msp.lwc.TextComponent::getString({this:?}) -> {text:?}");
        Ok(text)
    }

    async fn set_string(jvm: &Jvm, _: &mut WieJvmContext, mut this: ClassInstanceRef<TextComponent>, data: ClassInstanceRef<String>) -> JvmResult<()> {
        // TextFieldComponent/TextBoxComponent constructors allow null initial
        // data. Normalize null to the empty string for the internal state.
        let data = if data.is_null() {
            JavaLangString::from_rust_string(jvm, "").await?.into()
        } else {
            data
        };
        jvm.put_field(&mut this, "text", "Ljava/lang/String;", data).await?;
        tracing::info!("[PHASE8_56_WIPI_TEXT_COMPONENT] op=set_string");
        Ok(())
    }
}
