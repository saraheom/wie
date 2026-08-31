use alloc::vec;

use java_class_proto::JavaMethodProto;
use java_constants::MethodAccessFlags;
use jvm::{Jvm, Result as JvmResult};

use wie_jvm_support::{WieJavaClassProto, WieJvmContext};

// class org.kwis.msf.io.Network
pub struct Network;

impl Network {
    pub fn as_proto() -> WieJavaClassProto {
        WieJavaClassProto {
            name: "org/kwis/msf/io/Network",
            parent_class: Some("java/lang/Object"),
            interfaces: vec![],
            methods: vec![
                JavaMethodProto::new("connect", "()I", Self::connect, MethodAccessFlags::NATIVE | MethodAccessFlags::STATIC),
                JavaMethodProto::new(
                    "disconnect",
                    "()V",
                    Self::disconnect,
                    MethodAccessFlags::NATIVE | MethodAccessFlags::STATIC,
                ),
            ],
            fields: vec![],
            access_flags: Default::default(),
        }
    }

    async fn connect(_: &Jvm, context: &mut WieJvmContext) -> JvmResult<i32> {
        if context.system().aid() == "00026DBF" && context.system().pid() == "PD112525" {
            tracing::warn!("[PHASE8_62_OZ_NETWORK_CONNECT] org.kwis.msf.io.Network::connect() -> -1 (current generic WIE offline stub)");
        } else {
            tracing::warn!("stub org.kwis.msf.io.Network::connect()");
        }
        Ok(-1)
    }

    async fn disconnect(_: &Jvm, _: &mut WieJvmContext) -> JvmResult<()> {
        tracing::warn!("stub org.kwis.msf.io.Network::disconnect()");

        Ok(())
    }
}
