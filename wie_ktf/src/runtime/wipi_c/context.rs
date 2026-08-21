use alloc::{boxed::Box, vec, vec::Vec};

use jvm::{
    Jvm,
    runtime::{JavaIoInputStream, JavaLangClassLoader},
};
use wipi_types::wipic::{WIPICIndirectPtr, WIPICWord};

use wie_backend::{AsyncCallable, Event, Instant, System};
use wie_core_arm::{Allocator, ArmCore};
use wie_jvm_support::JvmSupport;
use wie_util::{ByteRead, ByteWrite, Result, WieError, read_generic, write_generic};
use wie_wipi_c::{WIPICContext, WIPICMethodBody};

#[derive(Clone)]
pub struct KtfWIPICContext {
    core: ArmCore,
    system: System,
    jvm: Jvm, // We need jvm to access resource in jvm. TODO is there better way to do this?
}

impl KtfWIPICContext {
    pub fn new(core: ArmCore, system: System, jvm: Jvm) -> Self {
        Self { core, system, jvm }
    }
}

#[async_trait::async_trait]
impl WIPICContext for KtfWIPICContext {
    fn alloc_raw(&mut self, size: WIPICWord) -> Result<WIPICWord> {
        Allocator::alloc(&mut self.core, size)
    }

    fn alloc(&mut self, size: WIPICWord) -> Result<WIPICIndirectPtr> {
        let ptr = Allocator::alloc(&mut self.core, size + 12)?; // all allocation has indirect pointer
        write_generic(&mut self.core, ptr, ptr + 4)?;
        write_generic(&mut self.core, ptr + 4, size)?;

        Ok(WIPICIndirectPtr(ptr))
    }

    fn free(&mut self, memory: WIPICIndirectPtr) -> Result<()> {
        let size: u32 = read_generic(&self.core, memory.0 + 4)?;
        Allocator::free(&mut self.core, memory.0, size + 12)?;

        Ok(())
    }

    fn free_raw(&mut self, address: WIPICWord, size: WIPICWord) -> Result<()> {
        Allocator::free(&mut self.core, address, size)?;

        Ok(())
    }

    fn data_ptr(&self, memory: WIPICIndirectPtr) -> Result<WIPICWord> {
        let base: WIPICWord = read_generic(&self.core, memory.0)?;

        Ok(base + 8) // all data has offset of 8 bytes
    }

    fn system(&mut self) -> &mut System {
        &mut self.system
    }

    fn debug_cpu_context(&self) -> Option<[WIPICWord; 17]> {
        let c = self.core.save_context();
        Some([
            c.r0, c.r1, c.r2, c.r3,
            c.r4, c.r5, c.r6, c.r7,
            c.r8, c.sb, c.sl, c.fp,
            c.ip, c.sp, c.lr, c.pc, c.cpsr,
        ])
    }

    async fn call_function(&mut self, address: WIPICWord, args: &[WIPICWord]) -> Result<WIPICWord> {
        self.core.run_function(address, args).await
    }

    fn spawn(&mut self, callback: WIPICMethodBody) -> Result<()> {
        struct SpawnProxy {
            context: KtfWIPICContext,
            callback: WIPICMethodBody,
        }

        impl AsyncCallable<Result<()>> for SpawnProxy {
            async fn call(mut self) -> Result<()> {
                self.context.jvm.attach_thread(None).await.unwrap();
                self.callback.call(&mut self.context, Box::new([])).await?;
                self.context.jvm.detach_thread().unwrap();

                Ok(())
            }
        }

        self.system.spawn(SpawnProxy {
            context: self.clone(),
            callback,
        });

        Ok(())
    }

    async fn get_resource_size(&self, name: &str) -> Result<Option<usize>> {
        let class_loader = JavaLangClassLoader::get_system_class_loader(&self.jvm)
            .await
            .map_err(|err| WieError::FatalError(alloc::format!("Failed to get class loader for resource {name:?}: {err:?}")))?;
        let stream = match JavaLangClassLoader::get_resource_as_stream(&self.jvm, &class_loader, name).await {
            Ok(stream) => stream,
            Err(err) => {
                tracing::error!("Java exception while opening resource for size query: name={name:?}, error={err:?}");
                return Err(JvmSupport::to_wie_err(&self.jvm, err).await);
            }
        };

        if let Some(stream) = stream {
            let available: i32 = match self.jvm.invoke_virtual(&stream, "available", "()I", ()).await {
                Ok(available) => available,
                Err(err) => return Err(JvmSupport::to_wie_err(&self.jvm, err).await),
            };
            drop(stream);
            match self.jvm.collect_garbage() {
                Ok(_) => {}
                Err(err) => return Err(JvmSupport::to_wie_err(&self.jvm, err).await),
            }
            return Ok(Some(available as usize));
        }

        match self.jvm.collect_garbage() {
            Ok(_) => {}
            Err(err) => return Err(JvmSupport::to_wie_err(&self.jvm, err).await),
        }

        // Phase 8.4 — KTF packaged-database filesystem fallback.
        //
        // KTF archives can carry preinstalled database records outside the
        // JAR under P/ or p/.  The KTF emulator already exposes archive files
        // through System::filesystem(), but this resource path previously
        // queried only the Java class loader.  As a result, a file such as
        // Inotia 2's p/i_pack.dat was invisible to MC_dbOpenDataBase even
        // though it was present in the package.
        //
        // Try the normalized/root form first, then both historical P/ cases.
        if let Some(size) = self.system.filesystem().size(name).await {
            tracing::info!(
                "[KTF_RESOURCE_FALLBACK] size name={name} path={name} size={size}"
            );
            return Ok(Some(size));
        }

        let upper_path = alloc::format!("P/{name}");
        if let Some(size) = self.system.filesystem().size(&upper_path).await {
            tracing::info!(
                "[KTF_RESOURCE_FALLBACK] size name={name} path={upper_path} size={size}"
            );
            return Ok(Some(size));
        }

        let lower_path = alloc::format!("p/{name}");
        if let Some(size) = self.system.filesystem().size(&lower_path).await {
            tracing::info!(
                "[KTF_RESOURCE_FALLBACK] size name={name} path={lower_path} size={size}"
            );
            return Ok(Some(size));
        }

        Ok(None)
    }

    async fn read_resource(&self, name: &str) -> Result<Vec<u8>> {
        let class_loader = JavaLangClassLoader::get_system_class_loader(&self.jvm)
            .await
            .map_err(|err| WieError::FatalError(alloc::format!("Failed to get class loader for resource {name:?}: {err:?}")))?;
        let stream = match JavaLangClassLoader::get_resource_as_stream(&self.jvm, &class_loader, name).await {
            Ok(stream) => stream,
            Err(err) => {
                tracing::error!("Java exception while opening resource for read: name={name:?}, error={err:?}");
                return Err(JvmSupport::to_wie_err(&self.jvm, err).await);
            }
        };

        if let Some(stream) = stream {
            let data = match JavaIoInputStream::read_until_end(&self.jvm, &stream).await {
                Ok(data) => data,
                Err(err) => {
                    tracing::error!("Java exception while reading resource: name={name:?}, error={err:?}");
                    return Err(JvmSupport::to_wie_err(&self.jvm, err).await);
                }
            };
            drop(stream);
            match self.jvm.collect_garbage() {
                Ok(_) => {}
                Err(err) => return Err(JvmSupport::to_wie_err(&self.jvm, err).await),
            }
            return Ok(data);
        }

        match self.jvm.collect_garbage() {
            Ok(_) => {}
            Err(err) => return Err(JvmSupport::to_wie_err(&self.jvm, err).await),
        }

        // Keep this lookup order identical to get_resource_size().  The
        // database layer calls size first and read second, so resolving the
        // same archive-backed object in both operations is essential.
        let mut resolved_path: Option<alloc::string::String> = None;
        let mut resolved_size: Option<usize> = None;

        if let Some(size) = self.system.filesystem().size(name).await {
            resolved_path = Some(name.into());
            resolved_size = Some(size);
        } else {
            let upper_path = alloc::format!("P/{name}");
            if let Some(size) = self.system.filesystem().size(&upper_path).await {
                resolved_path = Some(upper_path);
                resolved_size = Some(size);
            } else {
                let lower_path = alloc::format!("p/{name}");
                if let Some(size) = self.system.filesystem().size(&lower_path).await {
                    resolved_path = Some(lower_path);
                    resolved_size = Some(size);
                }
            }
        }

        let (path, size) = match (resolved_path, resolved_size) {
            (Some(path), Some(size)) => (path, size),
            _ => {
                return Err(WieError::FatalError(alloc::format!(
                    "Resource disappeared before read: {name:?}"
                )));
            }
        };

        let mut data = vec![0; size];
        let read = self
            .system
            .filesystem()
            .read(&path, 0, size, &mut data)
            .await
            .unwrap_or(0);
        data.truncate(read);

        tracing::info!(
            "[KTF_RESOURCE_FALLBACK] read name={name} path={path} expected={size} read={read}"
        );

        Ok(data)
    }

    fn set_timer(&mut self, due: Instant, callback: WIPICMethodBody) {
        let context = self.clone();

        self.system().event_queue().push(Event::timer(due, move || {
            let mut context = context.clone();

            async move {
                callback.call(&mut context, Box::new([])).await?;
                Ok(())
            }
        }))
    }
}

impl ByteRead for KtfWIPICContext {
    fn read_bytes(&self, address: WIPICWord, result: &mut [u8]) -> wie_util::Result<usize> {
        self.core.read_bytes(address, result)
    }
}

impl ByteWrite for KtfWIPICContext {
    fn write_bytes(&mut self, address: WIPICWord, data: &[u8]) -> wie_util::Result<()> {
        self.core.write_bytes(address, data)
    }
}
