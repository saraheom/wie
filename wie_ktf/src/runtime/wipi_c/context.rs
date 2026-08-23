use alloc::{boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec, vec::Vec};

use jvm::{
    Jvm,
    runtime::{JavaIoInputStream, JavaLangClassLoader},
};
use spin::Mutex;
use wipi_types::wipic::{WIPICIndirectPtr, WIPICWord};

use wie_backend::{AsyncCallable, Event, Instant, System};
use wie_core_arm::{Allocator, ArmCore};
use wie_jvm_support::JvmSupport;
use wie_util::{ByteRead, ByteWrite, Result, WieError, read_generic, write_generic};
use wie_wipi_c::{WIPICContext, WIPICMethodBody};

pub type KtfResourceCache = Arc<Mutex<BTreeMap<String, Vec<u8>>>>;

#[derive(Clone)]
pub struct KtfWIPICContext {
    core: ArmCore,
    system: System,
    jvm: Jvm, // We need jvm to access resource in jvm. TODO is there better way to do this?
    // Phase 8.19 — shared across cloned KTF contexts for one app launch.
    // Inotia 2 repeatedly asks the packaged filesystem for the same multi-MB
    // canonical install resources during map/skill transitions.  The backend
    // filesystem bridge is much more expensive than a guest-side Vec clone, so
    // cache only these immutable packaged resources for the exact title. The
    // tiny appinfo/envinfo/cert fallbacks are included too because each native
    // filesystem bridge crossing costs about 100 ms on iOS and they are hit by
    // menus/startup even though their payloads are tiny.
    resource_cache: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
}

impl KtfWIPICContext {
    pub fn new_resource_cache() -> KtfResourceCache {
        Arc::new(Mutex::new(BTreeMap::new()))
    }

    pub fn with_resource_cache(
        core: ArmCore,
        system: System,
        jvm: Jvm,
        resource_cache: KtfResourceCache,
    ) -> Self {
        Self {
            core,
            system,
            jvm,
            resource_cache,
        }
    }

    fn is_inotia2_ktf(&self) -> bool {
        self.system.aid() == "010100D5" && self.system.pid() == "PD007974"
    }

    fn is_inotia2_hot_resource(&self, name: &str) -> bool {
        self.is_inotia2_ktf()
            && matches!(
                name,
                "i_pack.dat"
                    | "eventdata.dat"
                    | "filetext.dat"
                    | "i_mapfeature.dat"
                    | "i_tile.dat"
                    | "appinfo.dat"
                    | "envinfo.dat"
                    | "cert.c2s"
            )
    }

    fn inotia2_hot_resource_cached_len(&self, name: &str) -> Option<usize> {
        self.resource_cache.lock().get(name).map(Vec::len)
    }

    async fn read_inotia2_hot_resource_cached(&self, name: &str) -> Result<Option<Vec<u8>>> {
        if !self.is_inotia2_hot_resource(name) {
            return Ok(None);
        }

        // Clone while holding the tiny spin lock, then release it before any
        // async work.  This cache is shared by timer/spawn context clones.
        if let Some(data) = self.resource_cache.lock().get(name).cloned() {
            tracing::debug!(
                "[PHASE8_19_INOTIA2_RESOURCE_CACHE] HIT name={name} size={}",
                data.len()
            );
            return Ok(Some(data));
        }

        // The known KTF package carries the canonical expanded files under p/.
        // Keep the historical resolution order for robustness, but only this
        // exact title/resource set reaches the fast path.
        let candidates = [
            String::from(name),
            alloc::format!("P/{name}"),
            alloc::format!("p/{name}"),
        ];

        for path in candidates {
            let Some(size) = self.system.filesystem().size(&path).await else {
                continue;
            };

            let mut data = vec![0; size];
            let read = self
                .system
                .filesystem()
                .read(&path, 0, size, &mut data)
                .await
                .unwrap_or(0);
            data.truncate(read);

            if read == size {
                self.resource_cache
                    .lock()
                    .insert(String::from(name), data.clone());
                tracing::info!(
                    "[PHASE8_19_INOTIA2_RESOURCE_CACHE] LOAD name={name} path={path} size={size}"
                );
            } else {
                tracing::warn!(
                    "[PHASE8_19_INOTIA2_RESOURCE_CACHE] short read name={name} path={path} expected={size} read={read}; not cached"
                );
            }

            return Ok(Some(data));
        }

        Ok(None)
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
        // Phase 8.19 — for Inotia 2's immutable packaged install resources,
        // load/cache before entering the JVM class-loader fallback.  This turns
        // repeated size+read archive crossings during map/skill transitions
        // into in-memory lookups for the remainder of the launch.
        if self.is_inotia2_hot_resource(name) {
            // Do not clone a multi-megabyte cached Vec merely to answer a size
            // query. The common map/skill path asks size before read.
            if let Some(size) = self.inotia2_hot_resource_cached_len(name) {
                return Ok(Some(size));
            }
            if let Some(data) = self.read_inotia2_hot_resource_cached(name).await? {
                return Ok(Some(data.len()));
            }
        }

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
        if self.is_inotia2_hot_resource(name) {
            if let Some(data) = self.read_inotia2_hot_resource_cached(name).await? {
                return Ok(data);
            }
        }

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
