use alloc::{borrow::ToOwned, boxed::Box, format, str, string::String, vec, vec::Vec};
use core::mem::size_of;

use bytemuck::{Pod, Zeroable};

use wipi_types::wipic::WIPICWord;

use wie_backend::Database;
use wie_util::{Result, read_generic, read_null_terminated_string_bytes, write_generic};

use crate::context::WIPICContext;

/// Per-handle state for KTF's stream-style database API.
///
/// KTF's `stream_read` / `stream_write` slots behave like a record-scoped
/// `fread` / `fwrite` pair rather than the standard WIPI record-by-id API
/// — the same record id 1 is walked sequentially with implicit cursors.
/// The original interface field names (`read_record_single`,
/// `write_record_single`) were a pre-disassembly guess; the impl-side names
/// `stream_read` / `stream_write` reflect the verified semantics.
///
/// The handle, including its read/write cursors and the in-memory mirror
/// of record 1, lives entirely in emulated memory: the `DatabaseHandle`
/// struct sits at the pointer returned from `open_database`, and the
/// mirror itself is a separate guest-heap allocation referenced by
/// `buffer_ptr`. Every op reads the struct, mutates it, writes it back —
/// no host-side global state.
///
/// `select_record` with a non-zero recid is treated as a seek: KTF apps
/// use slot 4 to position the cursor at known byte offsets within the
/// single backing record, e.g. for multi-slot save files.
#[derive(Pod, Zeroable, Copy, Clone)]
#[repr(C)]
struct DatabaseHandle {
    magic: u32,
    name: [u8; 32], // TODO hardcoded max size
    read_cursor: u32,
    write_cursor: u32,
    buffer_ptr: u32,
    buffer_len: u32,
    buffer_capacity: u32,
    // Host-side metadata stored in the opaque guest handle. KTF callers only
    // pass this pointer back to WIPI APIs; they do not inspect its layout.
    // Phase 8.18 uses the original open mode to defer writeback only for
    // Inotia 2's static mode-4 installation resources.
    open_mode: i32,
}

const MIN_BUFFER_CAPACITY: u32 = 64;
const KTF_DATABASE_STORAGE_LIMIT: u64 = 16 * 1024 * 1024;
// "MCDB" — sentinel at the start of the handle struct so we can distinguish
// a real DB handle pointer from an unrelated guest pointer (e.g. a C-string
// name pointer that KTF's slot 6 passes through the same SVC argument slot).
const DATABASE_HANDLE_MAGIC: u32 = 0x4D434442;
const MAX_NAME_LEN: usize = 31; // leave a byte for null terminator inside the 32-byte field

// Phase 8.14 — Inotia 2 ships four generated database caches twice: a
// compact source copy inside 010100D5.jar and the already-expanded KTF
// database image under p/.  KTF opens these files with mode 4 when rebuilding
// them.  Preserving an existing record on CREATE caused each launch to append
// another expanded copy after the compact seed.  After nine rebuilds, for
// example, filetext.dat had grown from 93,067 bytes to 2,808,205 bytes.
// Besides wasting memory, the stale compact header at byte zero made the game
// compute bogus offsets, which manifested as broken strings and eventually an
// invalid-memory-access crash in the key-setting screen.
//
// Keep this list exact-title/specific-data-only.  i_pack.dat already has its
// own Phase 8.9 CREATE fix and is intentionally not treated as a prebuilt
// cache here.
fn is_inotia2_generated_cache(name: &str) -> bool {
    matches!(
        name,
        "eventdata.dat" | "filetext.dat" | "i_mapfeature.dat" | "i_tile.dat"
    )
}

// Phase 8.17 — canonical installed lengths from this exact PD007974 package.
// If a persistent resource already has one of these lengths on a normal open,
// there is no reason to reopen/read the large archive-backed p/ copy merely to
// rediscover the same bytes.  This removes repeated 100ms-class filesystem
// reads during map/menu transitions while retaining the Phase 8.14 repair path
// whenever a record length is wrong.
fn inotia2_canonical_installed_len(name: &str) -> Option<usize> {
    match name {
        "i_pack.dat" => Some(1_489_150),
        "eventdata.dat" => Some(119_634),
        "filetext.dat" => Some(301_682),
        "i_mapfeature.dat" => Some(44_928),
        "i_tile.dat" => Some(194_928),
        _ => None,
    }
}

// Phase 8.19 — the four generated caches acquire an eight-byte runtime footer
// immediately after the install builder closes them. Phase 8.18's trace proved
// this is deterministic: e.g. filetext.dat closes at 301,682, then the title
// appends two four-byte values and leaves a valid 301,690-byte record. The old
// Phase 8.14 repair logic treated that legitimate footer as corruption and
// stripped it back to the packaged p/ length. That both adds transition I/O and
// is a strong candidate for why the native installer believes it must run on
// every launch. Accept only the exact base or base+8 forms; multi-copy growth
// (the original 2.8 MiB corruption) is still rejected and repaired.
fn inotia2_valid_installed_len(name: &str, len: usize) -> bool {
    let Some(base) = inotia2_canonical_installed_len(name) else {
        return false;
    };
    len == base || (is_inotia2_generated_cache(name) && len == base.saturating_add(8))
}

fn is_inotia2_static_install_resource(name: &str) -> bool {
    inotia2_canonical_installed_len(name).is_some()
}

fn inotia2_host_db_cache_key(name: &str) -> String {
    format!("db:{name}")
}

fn bm3_host_db_cache_key(name: &str) -> String {
    format!("phase8_82_bm3_db:{name}")
}

async fn read_inotia2_prebuilt_cache(
    context: &mut dyn WIPICContext,
    name: &str,
) -> Result<Option<Vec<u8>>> {
    if !is_inotia2_generated_cache(name) {
        return Ok(None);
    }

    // Clone the overlay so no System borrow is held across filesystem awaits.
    // The outer KTF archive stores the canonical expanded DBs under p/.
    let filesystem = context.system().filesystem().clone();
    for prefix in ["p/", "P/"] {
        let path = format!("{prefix}{name}");
        let Some(size) = filesystem.size(&path).await else {
            continue;
        };

        let mut data = vec![0u8; size];
        let Some(read) = filesystem.read(&path, 0, size, &mut data).await else {
            continue;
        };
        if read == size {
            return Ok(Some(data));
        }

        tracing::warn!(
            "[PHASE8_14_INOTIA2_CACHE_RESTORE] short archive read path={path} expected={size} read={read}"
        );
    }

    Ok(None)
}

pub async fn open_database(context: &mut dyn WIPICContext, ptr_name: WIPICWord, mode: i32, r#type: i32) -> Result<i32> {
    tracing::debug!("MC_dbOpenDataBase({ptr_name:#x}, {mode}, {type})");

    // Guest-provided C string — invalid UTF-8 must not bring down the
    // emulator. Treat it as a bad parameter and return -22, matching the
    // fail-soft behaviour of the other name-keyed entry points in this
    // file (`stat_by_name_ktf`, `exists_database_ktf`).
    let Ok(name) = String::from_utf8(read_null_terminated_string_bytes(context, ptr_name)?) else {
        tracing::warn!("MC_dbOpenDataBase: invalid utf8 name @ {ptr_name:#x}");
        return Ok(-22);
    };

    // Validate before any repository side effects. Mode 4 deletes record 1
    // up front, so a too-long name reaching that path would wipe data we
    // can't open a handle for anyway.
    if name.len() > MAX_NAME_LEN {
        tracing::warn!("MC_dbOpenDataBase: name {name:?} too long ({} > {MAX_NAME_LEN})", name.len());
        return Ok(-22); // M_E_BADRECID — closest WIPI parameter-error idiom in this file
    }

    // Capture the title identifiers before resource I/O without holding a mutable
    // System borrow across another context call.
    let (pid, aid) = {
        let system = context.system();
        (system.pid().to_owned(), system.aid().to_owned())
    };

    // Phase 8.82 — Blade Master 3 can deadlock inside the browser IndexedDB
    // repository while opening its first LGT database. Keep its database
    // working set in the existing per-emulator host blob cache for this
    // session. Packaged resources remain valid seeds; missing READ opens fail
    // immediately with M_E_NOENT instead of awaiting IndexedDB indefinitely.
    let phase882_bm3 = pid == "PD109653" && aid == "000262F4";
    let bm3_cache_key = phase882_bm3.then(|| bm3_host_db_cache_key(&name));
    if mode == 4 {
        if let Some(key) = bm3_cache_key.as_deref() {
            context.host_blob_cache_remove(key);
        }
    }
    let bm3_cached = if mode != 4 {
        bm3_cache_key.as_deref().and_then(|key| context.host_blob_cache_get(key))
    } else {
        None
    };
    if phase882_bm3 {
        tracing::info!(
            "[PHASE8_82_BM3_DB_OPEN_BEGIN] name={name:?} mode={mode} type={type} cache_hit={} source=session_memory",
            bm3_cached.is_some()
        );
    }

    // Phase 8.21 — cache the *installed persistent* static records as well as
    // the packaged p/ resources. Phase 8.20 fixed the shared packaged cache,
    // but every normal DB reopen could still cross IndexedDB for exists/open/get
    // before copying the same immutable i_pack/event/tile data into a guest
    // handle. Keep this second namespace in the same per-launch KTF Arc.
    // CREATE explicitly invalidates it; write/close paths repopulate it with
    // the exact current record, including the legitimate +8 installed footer.
    let inotia2_db_cache_key = if pid == "PD007974"
        && aid == "010100D5"
        && is_inotia2_static_install_resource(&name)
    {
        Some(inotia2_host_db_cache_key(&name))
    } else {
        None
    };
    if mode == 4 {
        if let Some(key) = inotia2_db_cache_key.as_deref() {
            context.host_blob_cache_remove(key);
        }
    }
    let cached_installed = if mode != 4 {
        inotia2_db_cache_key
            .as_deref()
            .and_then(|key| context.host_blob_cache_get(key))
    } else {
        None
    };
    if let Some(data) = cached_installed.as_ref() {
        tracing::debug!(
            "[PHASE8_21_INOTIA2_DB_CACHE] HIT name={name} len={} -> skip IndexedDB exists/open/get",
            data.len()
        );
    }

    // Phase 8.17 — determine persistence before touching archive resources.
    // Phase 8.16 logs showed that PD007974 repeatedly reread the complete
    // 1.49 MiB p/i_pack.dat (and generated cache snapshots) even when the
    // persistent database was already canonical.  The archive filesystem path
    // costs roughly 100ms per size/read operation on the iOS build and is hit
    // around map/menu transitions.  For a normal open of a verified-length
    // installed record, skip those redundant archive reads entirely.
    let exists = if phase882_bm3 {
        bm3_cached.is_some()
    } else if cached_installed.is_some() {
        true
    } else {
        let system = context.system();
        system.platform().database_repository().exists(&name, &pid).await
    };

    // Phase 8.18 — these are immutable installation/resource records for this
    // exact Inotia 2 build. The game legitimately opens them with CREATE on
    // startup, but flushing every tiny stream write to the iOS repository
    // causes severe write amplification. We still execute the guest rebuild
    // routine (it initializes required in-memory tables), while buffering its
    // writes in the guest handle and committing once at close.
    let inotia2_static_create = pid == "PD007974"
        && aid == "010100D5"
        && mode == 4
        && is_inotia2_static_install_resource(&name);

    let mut inotia2_resource_fastpath = false;
    if pid == "PD007974" && aid == "010100D5" && mode != 4 && exists {
        if let Some(expected_len) = inotia2_canonical_installed_len(&name) {
            let actual_len = if let Some(data) = cached_installed.as_ref() {
                data.len()
            } else {
                let system = context.system();
                let mut db = system.platform().database_repository().open(&name, &pid).await;
                db.get(1).await.map(|data| data.len()).unwrap_or(0)
            };
            if inotia2_valid_installed_len(&name, actual_len) {
                inotia2_resource_fastpath = true;
                tracing::debug!(
                    "[PHASE8_19_INOTIA2_INSTALLED_FASTPATH] name={name} len={actual_len} base={expected_len} -> preserve valid installed record and skip redundant archive read"
                );
            }
        }
    }

    let packaged = if inotia2_resource_fastpath || (inotia2_static_create && exists) {
        None
    } else {
        read_packaged_database(context, &name).await?
    };
    if phase882_bm3 {
        tracing::info!(
            "[PHASE8_82_BM3_DB_SOURCE] name={name:?} mode={mode} session_len={} packaged_len={}",
            bm3_cached.as_ref().map(|x| x.len()).unwrap_or(0),
            packaged.as_ref().map(|x| x.len()).unwrap_or(0)
        );
    }
    let inotia2_prebuilt_cache = if pid == "PD007974"
        && is_inotia2_generated_cache(&name)
        && !inotia2_resource_fastpath
        && (!inotia2_static_create || !exists)
    {
        read_inotia2_prebuilt_cache(context, &name).await?
    } else {
        None
    };

    // Phase 8.13: do not fabricate tcert.c2s. The Phase 8.12 alias let the
    // obsolete carrier certificate validator consume cert.c2s as if it were
    // its companion certificate, after which PD007974 stalled. The exact
    // legacy validator is bypassed at native-load time instead, preserving the
    // normal success continuation without inventing certificate contents.

    let packaged_len = packaged.as_ref().map(|data| data.len()).unwrap_or(0);

    let system = context.system();
    if pid == "PD005362" {
        tracing::debug!("[PHASE7_21] Inotia1 KTF record-length seek-return fix active");
    }

    if pid == "PD007974" {
        tracing::debug!(
            "[PHASE8_3] Inotia2 i_pack database trace active"
        );
        tracing::debug!(
            "[INOTIA2_DB] OPEN_REQUEST name={name} mode={mode} type={type} exists={exists} packaged_len={packaged_len}"
        );
    }

    if !exists && packaged.is_none() && mode == 1 {
        if phase882_bm3 {
            tracing::info!("[PHASE8_82_BM3_DB_OPEN_RETURN] name={name:?} mode={mode} result=-12 reason=missing-no-indexeddb");
        }
        if pid == "PD007974" {
            tracing::debug!(
                "[INOTIA2_DB] OPEN_RESULT name={name} mode={mode} -> -12 (NOENT)"
            );
        }
        return Ok(-12); // M_E_NOENT
    }

    // Mode 4 (`MC_DB_CREATE`) is true create/truncate semantics.  Phase 7.14
    // temporarily preserved an existing Inotia 1 record here; the exported
    // before/after quest snapshots proved that doing so can leave bytes from
    // the previous save generation in the record.  Revert to strict CREATE:
    // delete record 1 first, then let subsequent writes rebuild it.
    let initial: Vec<u8> = if let Some(data) = bm3_cached.clone() {
        data
    } else if let Some(data) = cached_installed.clone() {
        data
    } else if exists {
        let mut db = system.platform().database_repository().open(&name, &pid).await;
        let inotia2_ipack_create =
            pid == "PD007974" && name == "i_pack.dat" && mode == 4;
        let inotia2_cache_create =
            pid == "PD007974" && is_inotia2_generated_cache(&name) && mode == 4;

        if inotia2_static_create {
            let old_len = db.get(1).await.map(|x| x.len()).unwrap_or(0);
            let canonical_len = inotia2_canonical_installed_len(&name).unwrap_or(0);
            tracing::info!(
                "[PHASE8_18_INOTIA2_INSTALL_WRITEBACK] OPEN name={name} mode=CREATE existing={old_len} canonical={canonical_len} -> preserve repository copy; rebuild buffered in guest memory"
            );
            // Deliberately do NOT delete record 1 here. If the title/app is
            // interrupted mid-rebuild, the last known-good canonical resource
            // remains available on the next launch.
            Vec::new()
        } else if mode == 4 && (packaged.is_none() || inotia2_ipack_create || inotia2_cache_create) {
            let old_len = db.get(1).await.map(|x| x.len()).unwrap_or(0);

            if pid == "PD005362" {
                tracing::debug!(
                    "[INOTIA1_SAVE] OPEN db={name} mode=CREATE existing={old_len} -> truncate"
                );
            }

            if inotia2_ipack_create {
                tracing::info!(
                    "[PHASE8_9_IPACK_CREATE] Inotia2 i_pack.dat CREATE existing={old_len} packaged_len={packaged_len} -> truncate persistent record before rebuild"
                );
            }
            if inotia2_cache_create {
                let canonical_len = inotia2_prebuilt_cache.as_ref().map(|data| data.len()).unwrap_or(0);
                tracing::info!(
                    "[PHASE8_14_INOTIA2_CACHE_CREATE] name={name} existing={old_len} compact_packaged={packaged_len} canonical_expanded={canonical_len} -> truncate before rebuild"
                );
            }

            db.delete(1).await;
            Vec::new()
        } else if let Some(data) = db.get(1).await {
            if pid == "PD005362" {
                tracing::debug!(
                    "[INOTIA1_SAVE] OPEN db={name} mode={mode} existing={} -> preserve",
                    data.len()
                );
            }

            let mut data = data;

            // Phase 8.16: do not overwrite appinfo.dat with the bundled copy.
            // Phase 8.15 proved that appinfo is not the install/rebuild gate: the
            // game still entered the rebuild path after the replacement.  The
            // actual redundant verification branch is now patched after its
            // i_pack loader has run (wie_ktf::runtime::init).

            // Repair records polluted by the pre-8.14 CREATE behavior without
            // requiring the user to erase all saved state.  Size is a safe
            // discriminator here: these four files are static generated caches,
            // while the canonical expanded copy is shipped in p/ by the title.
            if pid == "PD007974" && mode != 4 {
                if let Some(prebuilt) = inotia2_prebuilt_cache.as_ref() {
                    if !inotia2_valid_installed_len(&name, data.len()) {
                        tracing::info!(
                            "[PHASE8_14_INOTIA2_CACHE_RESTORE] name={name} persistent_len={} canonical_len={} -> restore p/ snapshot",
                            data.len(),
                            prebuilt.len()
                        );
                        db.set(1, prebuilt).await;
                        prebuilt.clone()
                    } else {
                        if data.len() == prebuilt.len().saturating_add(8) {
                            tracing::debug!(
                                "[PHASE8_19_INOTIA2_INSTALL_FOOTER] name={name} len={} -> preserve valid +8 runtime footer",
                                data.len()
                            );
                        }
                        data
                    }
                } else {
                    data
                }
            } else {
                data
            }
        } else if let Some(prebuilt) = inotia2_prebuilt_cache.as_ref() {
            db.set(1, prebuilt).await;
            prebuilt.clone()
        } else if let Some(data) = packaged {
            db.set(1, &data).await;
            data
        } else {
            Vec::new()
        }
    } else if phase882_bm3 && mode != 4 {
        packaged.unwrap_or_default()
    } else if mode != 4 {
        if let Some(prebuilt) = inotia2_prebuilt_cache.as_ref() {
            let mut db = system.platform().database_repository().open(&name, &pid).await;
            tracing::info!(
                "[PHASE8_14_INOTIA2_CACHE_RESTORE] name={name} persistent=missing canonical_len={} -> seed p/ snapshot",
                prebuilt.len()
            );
            db.set(1, prebuilt).await;
            prebuilt.clone()
        } else if let Some(data) = packaged {
            let mut db = system.platform().database_repository().open(&name, &pid).await;
            db.set(1, &data).await;
            data
        } else {
            Vec::new()
        }
    } else if mode == 4 && inotia2_static_create {
        // Fresh imports have no persistent canonical copy to preserve yet.
        // Seed one once from the shipped expanded resource before allowing the
        // native installer to rebuild its in-memory working copy.
        let seed = if let Some(prebuilt) = inotia2_prebuilt_cache.as_ref() {
            Some(prebuilt.clone())
        } else {
            packaged.clone()
        };
        if let Some(seed) = seed {
            let mut db = system.platform().database_repository().open(&name, &pid).await;
            db.set(1, &seed).await;
            tracing::info!(
                "[PHASE8_18_INOTIA2_INSTALL_WRITEBACK] SEED name={name} canonical_len={} for fresh install",
                seed.len()
            );
        } else {
            system.platform().database_repository().open(&name, &pid).await;
        }
        Vec::new()
    } else if mode == 4 {
        if !phase882_bm3 {
            system.platform().database_repository().open(&name, &pid).await;
        }
        Vec::new()
    } else {
        Vec::new()
    };

    if mode != 4 {
        if let Some(key) = inotia2_db_cache_key.as_deref() {
            if !initial.is_empty() && inotia2_valid_installed_len(&name, initial.len()) {
                context.host_blob_cache_put(key, initial.clone());
                tracing::debug!(
                    "[PHASE8_21_INOTIA2_DB_CACHE] STORE name={name} len={} source=normal-open",
                    initial.len()
                );
            }
        }
    }

    // Phase 8.19 — keep Inotia 2's graphics settings user-controlled. Phase
    // 8.16 forced the low three envinfo bits off, which meant the UI could be
    // toggled on but the stored settings were silently rewritten. The newer
    // writeback/resource optimizations provide the performance baseline, so do
    // not mutate envinfo here.

    let name_bytes = name.as_bytes();

    let mut handle = DatabaseHandle {
        magic: DATABASE_HANDLE_MAGIC,
        name: [0; 32],
        read_cursor: 0,
        write_cursor: 0,
        buffer_ptr: 0,
        buffer_len: 0,
        buffer_capacity: 0,
        open_mode: mode,
    };
    handle.name[..name_bytes.len()].copy_from_slice(name_bytes);

    if !initial.is_empty() {
        let cap = (initial.len() as u32).max(MIN_BUFFER_CAPACITY);
        let buf_ptr = context.alloc_raw(cap)?;
        context.write_bytes(buf_ptr, &initial)?;
        handle.buffer_ptr = buf_ptr;
        handle.buffer_len = initial.len() as u32;
        handle.buffer_capacity = cap;
    } else if inotia2_static_create {
        // Avoid repeated power-of-two realloc/copy cycles while the native
        // installer reconstructs large static tables (especially i_pack.dat).
        let canonical_len = inotia2_canonical_installed_len(&name).unwrap_or(0);
        if canonical_len > 0 {
            let cap = (canonical_len as u32)
                .saturating_add(64)
                .max(MIN_BUFFER_CAPACITY);
            let buf_ptr = context.alloc_raw(cap)?;
            handle.buffer_ptr = buf_ptr;
            handle.buffer_capacity = cap;
            tracing::info!(
                "[PHASE8_18_INOTIA2_INSTALL_WRITEBACK] PREALLOC name={name} capacity={cap}"
            );
        }
    }

    let ptr_handle = context.alloc_raw(size_of::<DatabaseHandle>() as _)?;
    write_generic(context, ptr_handle, handle)?;

    tracing::debug!("Created database handle {ptr_handle:#x} for {name}");

    if pid == "PD007974" {
        let cpu = context.debug_cpu_context();
        if let Some(regs) = cpu {
            tracing::debug!(
                "[INOTIA2_DB] OPEN_RESULT name={name} mode={mode} handle={ptr_handle:#010x} initial_len={} buffer_ptr={:#010x} capacity={} lr={:#010x} pc={:#010x}",
                handle.buffer_len,
                handle.buffer_ptr,
                handle.buffer_capacity,
                regs[14],
                regs[15]
            );
        } else {
            tracing::debug!(
                "[INOTIA2_DB] OPEN_RESULT name={name} mode={mode} handle={ptr_handle:#010x} initial_len={} buffer_ptr={:#010x} capacity={}",
                handle.buffer_len,
                handle.buffer_ptr,
                handle.buffer_capacity
            );
        }
    }

    if phase882_bm3 {
        tracing::info!(
            "[PHASE8_82_BM3_DB_OPEN_RETURN] name={name:?} mode={mode} result={ptr_handle:#x} initial_len={} source=session_memory",
            handle.buffer_len
        );
    }

    Ok(ptr_handle as _)
}

pub async fn close_database(context: &mut dyn WIPICContext, db_id: i32) -> Result<i32> {
    tracing::debug!("MC_dbCloseDataBase({db_id:#x})");

    let Some(handle) = load_handle(context, db_id)? else {
        return Ok(-25); // M_E_INVALIDHANDLE
    };

    let db_name = handle_name(&handle).to_owned();
    let (pid, aid) = {
        let system = context.system();
        (system.pid().to_owned(), system.aid().to_owned())
    };

    if pid == "PD109653" && aid == "000262F4" {
        let mut snapshot = vec![0u8; handle.buffer_len as usize];
        if handle.buffer_ptr != 0 && handle.buffer_len > 0 {
            context.read_bytes(handle.buffer_ptr, &mut snapshot)?;
        }
        let key = bm3_host_db_cache_key(&db_name);
        context.host_blob_cache_put(&key, snapshot);
        tracing::info!(
            "[PHASE8_82_BM3_DB_CLOSE] name={db_name:?} mode={} len={} result=0 source=session_memory",
            handle.open_mode, handle.buffer_len
        );
    }

    if pid == "PD007974" {
        tracing::debug!(
            "[INOTIA2_DB] CLOSE db={} handle={db_id:#010x} len={} read_cursor={} write_cursor={} mode={}",
            db_name,
            handle.buffer_len,
            handle.read_cursor,
            handle.write_cursor,
            handle.open_mode
        );
    }

    let mut inotia2_cache_commit: Option<Vec<u8>> = None;

    // Phase 8.18 — Inotia 2's required startup rebuild is allowed to execute,
    // but its static mode-4 resources are write-back cached. Commit once here
    // instead of copying/flushing the entire growing record on every stream
    // write. Generated caches are normalized to the known-good expanded p/
    // snapshots, while i_pack uses the rebuilt bytes when they are canonical.
    if pid == "PD007974"
        && aid == "010100D5"
        && handle.open_mode == 4
        && is_inotia2_static_install_resource(&db_name)
    {
        let canonical_len = inotia2_canonical_installed_len(&db_name).unwrap_or(0);
        let mut rebuilt = vec![0u8; handle.buffer_len as usize];
        if handle.buffer_ptr != 0 && handle.buffer_len > 0 {
            context.read_bytes(handle.buffer_ptr, &mut rebuilt)?;
        }

        let (commit, source): (Vec<u8>, &str) = if is_inotia2_generated_cache(&db_name) {
            if let Some(prebuilt) = read_inotia2_prebuilt_cache(context, &db_name).await? {
                (prebuilt, "p/canonical-expanded")
            } else {
                (rebuilt, "rebuilt-no-prebuilt")
            }
        } else if rebuilt.len() == canonical_len {
            (rebuilt, "rebuilt")
        } else if let Some(packaged) = read_packaged_database(context, &db_name).await? {
            (packaged, "p/packaged-fallback")
        } else {
            (rebuilt, "rebuilt-noncanonical")
        };

        if !commit.is_empty() {
            let mut db = context
                .system()
                .platform()
                .database_repository()
                .open(&db_name, &pid)
                .await;
            db.set(1, &commit).await;
            inotia2_cache_commit = Some(commit.clone());
        }

        tracing::info!(
            "[PHASE8_18_INOTIA2_INSTALL_WRITEBACK] CLOSE name={db_name} rebuilt_len={} canonical={canonical_len} committed_len={} source={source}",
            handle.buffer_len,
            commit.len()
        );
    }

    // Phase 8.21 — keep the installed static-record mirror coherent across
    // repeated opens in this launch. CREATE uses the canonical commit selected
    // above; normal handles cache their exact current buffer (including +8
    // footers) at close. This avoids IndexedDB open/get on later map/skill use.
    if pid == "PD007974" && aid == "010100D5" && is_inotia2_static_install_resource(&db_name) {
        let cache_data = if let Some(commit) = inotia2_cache_commit.take() {
            Some(commit)
        } else if handle.buffer_ptr != 0 && handle.buffer_len > 0 {
            let mut snapshot = vec![0u8; handle.buffer_len as usize];
            context.read_bytes(handle.buffer_ptr, &mut snapshot)?;
            Some(snapshot)
        } else {
            None
        };
        if let Some(cache_data) = cache_data {
            let key = inotia2_host_db_cache_key(&db_name);
            context.host_blob_cache_put(&key, cache_data);
            tracing::debug!(
                "[PHASE8_21_INOTIA2_DB_CACHE] STORE name={db_name} source=close"
            );
        }
    }

    // Normal titles and non-static records remain write-through exactly as
    // before. The handle/buffer can now be released.
    if handle.buffer_ptr != 0 && handle.buffer_capacity > 0 {
        context.free_raw(handle.buffer_ptr, handle.buffer_capacity)?;
    }
    context.free_raw(db_id as _, size_of::<DatabaseHandle>() as _)?;

    Ok(0) // success
}

pub async fn list_record(context: &mut dyn WIPICContext, db_id: i32, buf_ptr: WIPICWord, buf_len: WIPICWord) -> Result<i32> {
    tracing::debug!("MC_dbListRecords({db_id:#x}, {buf_ptr:#x}, {buf_len})");

    let Some(db) = get_database_from_db_id(context, db_id).await? else {
        return Ok(-25); // M_E_INVALIDHANDLE
    };
    let ids = db.get_record_ids().await;

    if context.system().pid() == "PD005362" {
        tracing::info!(
            "[INOTIA1_META] LIST_RECORDS db={} count={} capacity_bytes={buf_len}",
            load_handle(context, db_id)?.as_ref().map(handle_name).unwrap_or("<invalid>"),
            ids.len()
        );
    }

    let mut cursor = 0;
    for &id in &ids {
        write_generic(context, buf_ptr + cursor, id)?;
        cursor += size_of::<WIPICWord>() as u32;
    }

    Ok(ids.len() as _)
}

/// Returns the database storage available to a KTF application.
///
/// Although the standard interface names function ID 12 `MC_dbListDataBase`,
/// KTF titles use its no-argument return value as an available-storage byte count.
/// Known callers reject values below 0x100 and 0x1200 respectively.
pub async fn list_databases(context: &mut dyn WIPICContext) -> Result<i32> {
    let system = context.system();
    let pid = system.pid().to_owned();
    let aid = system.aid().to_owned();
    if pid == "PD109653" && aid == "000262F4" {
        let available = KTF_DATABASE_STORAGE_LIMIT.min(i32::MAX as u64) as i32;
        tracing::info!("[PHASE8_82_BM3_DB_USAGE] result={available} source=session_memory");
        return Ok(available);
    }
    let usage = system.platform().database_repository().usage(&pid).await;
    let available = KTF_DATABASE_STORAGE_LIMIT.saturating_sub(usage).min(i32::MAX as u64) as i32;

    tracing::debug!("MC_dbListDataBase() = {available} (used={usage}, limit={KTF_DATABASE_STORAGE_LIMIT})");

    // Phase 8.1 — Inotia 2 KTF internal-heap state scan.
    //
    // Phase 8.0 proved the code signatures were correct, but its first-level
    // reads stopped at the PIC/GOT indirection.  The reported "capacity",
    // "used", etc. were therefore guest addresses of the real globals.
    //
    // The KTF Inotia 2 allocator at 0x125c54 uses:
    //   descriptor_limit = *(*GOT[0x1214])
    //   capacity         = *(*GOT[0x1218])
    //   heap_base        = *(*GOT[0x121c])
    //   block_table      =  *(GOT[0x1224])
    //   free_desc_head   = *(*GOT[0x1228])
    //   used             = *(*GOT[0x122c])
    //   alloc_head       = *(*GOT[0x1230])
    //   alloc_count      = *(*GOT[0x1234])
    //
    // This phase is still observational only.  It additionally walks both
    // descriptor chains to distinguish capacity exhaustion, descriptor
    // exhaustion, and fragmentation.
    if pid == "PD007974" {
        const INVALID: u32 = 0xffff_ffff;
        const GOT_BASE: u32 = 0x0019_24c4;

        let descriptor_limit_ptr: u32 =
            read_generic(context, GOT_BASE + 0x1214).unwrap_or(INVALID);
        let capacity_ptr: u32 =
            read_generic(context, GOT_BASE + 0x1218).unwrap_or(INVALID);
        let heap_base_ptr: u32 =
            read_generic(context, GOT_BASE + 0x121c).unwrap_or(INVALID);
        let heap_source: u32 =
            read_generic(context, GOT_BASE + 0x1220).unwrap_or(INVALID);
        let block_table: u32 =
            read_generic(context, GOT_BASE + 0x1224).unwrap_or(INVALID);
        let free_head_ptr: u32 =
            read_generic(context, GOT_BASE + 0x1228).unwrap_or(INVALID);
        let used_ptr: u32 =
            read_generic(context, GOT_BASE + 0x122c).unwrap_or(INVALID);
        let alloc_head_ptr: u32 =
            read_generic(context, GOT_BASE + 0x1230).unwrap_or(INVALID);
        let alloc_count_ptr: u32 =
            read_generic(context, GOT_BASE + 0x1234).unwrap_or(INVALID);

        let descriptor_limit: u32 =
            read_generic(context, descriptor_limit_ptr).unwrap_or(INVALID);
        let capacity: u32 =
            read_generic(context, capacity_ptr).unwrap_or(INVALID);
        let heap_base: u32 =
            read_generic(context, heap_base_ptr).unwrap_or(INVALID);
        let free_head: u32 =
            read_generic(context, free_head_ptr).unwrap_or(INVALID);
        let used: u32 =
            read_generic(context, used_ptr).unwrap_or(INVALID);
        let alloc_head: u32 =
            read_generic(context, alloc_head_ptr).unwrap_or(INVALID);
        let alloc_count: u32 =
            read_generic(context, alloc_count_ptr).unwrap_or(INVALID);

        // 0x193b00 is the GOT entry used by 0x1450d4 before it stores the
        // result of allocator(0x100).  Dereference it once more to get the
        // actual UI allocation result.
        let ui_ptr_global: u32 =
            read_generic(context, 0x0019_3b00).unwrap_or(INVALID);
        let ui_ptr: u32 =
            read_generic(context, ui_ptr_global).unwrap_or(INVALID);

        let free_bytes = if capacity != INVALID && used != INVALID {
            capacity.saturating_sub(used)
        } else {
            INVALID
        };

        let mut free_descriptor_count: u32 = 0;
        let mut free_descriptor_chain_ok = true;
        let mut free_idx = free_head;

        let scan_limit = if descriptor_limit != INVALID {
            descriptor_limit.min(0x4000)
        } else {
            0
        };

        while free_idx != INVALID && free_descriptor_count < scan_limit {
            if free_idx >= descriptor_limit {
                free_descriptor_chain_ok = false;
                break;
            }

            let entry = match block_table.checked_add(free_idx.saturating_mul(12)) {
                Some(v) => v,
                None => {
                    free_descriptor_chain_ok = false;
                    break;
                }
            };

            let next: u32 = match read_generic(context, entry + 8) {
                Ok(v) => v,
                Err(_) => {
                    free_descriptor_chain_ok = false;
                    break;
                }
            };

            free_descriptor_count += 1;

            // A proper singly-linked chain cannot need more nodes than the
            // descriptor table itself.  This also protects against cycles.
            if free_descriptor_count >= descriptor_limit && next != INVALID {
                free_descriptor_chain_ok = false;
                break;
            }

            free_idx = next;
        }

        let mut allocated_chain_count: u32 = 0;
        let mut allocated_chain_ok = true;
        let mut allocated_size_sum: u64 = 0;
        let mut largest_gap: u32 = 0;
        let mut monotonic = true;
        let mut alloc_idx = alloc_head;
        let mut cursor = heap_base;
        let pool_end = heap_base.checked_add(capacity).unwrap_or(INVALID);

        // Keep a compact sample of the first allocated descriptors.  This is
        // enough to verify the list structure without flooding the log.
        let mut sample: Vec<(u32, u32, u32, u32)> = Vec::new();

        while alloc_idx != INVALID && allocated_chain_count < scan_limit {
            if alloc_idx >= descriptor_limit {
                allocated_chain_ok = false;
                break;
            }

            let entry = match block_table.checked_add(alloc_idx.saturating_mul(12)) {
                Some(v) => v,
                None => {
                    allocated_chain_ok = false;
                    break;
                }
            };

            let addr: u32 = match read_generic(context, entry) {
                Ok(v) => v,
                Err(_) => {
                    allocated_chain_ok = false;
                    break;
                }
            };
            let size: u32 = match read_generic(context, entry + 4) {
                Ok(v) => v,
                Err(_) => {
                    allocated_chain_ok = false;
                    break;
                }
            };
            let next: u32 = match read_generic(context, entry + 8) {
                Ok(v) => v,
                Err(_) => {
                    allocated_chain_ok = false;
                    break;
                }
            };

            if sample.len() < 12 {
                sample.push((alloc_idx, addr, size, next));
            }

            if addr < cursor {
                monotonic = false;
            } else {
                largest_gap = largest_gap.max(addr.saturating_sub(cursor));
            }

            allocated_size_sum = allocated_size_sum.saturating_add(size as u64);

            let aligned_size = size.saturating_add(3) & !3;
            cursor = addr.saturating_add(aligned_size);

            allocated_chain_count += 1;

            if allocated_chain_count >= descriptor_limit && next != INVALID {
                allocated_chain_ok = false;
                break;
            }

            alloc_idx = next;
        }

        if pool_end != INVALID && cursor <= pool_end {
            largest_gap = largest_gap.max(pool_end - cursor);
        }

        let request = 0x100u32;
        let capacity_can_fit = free_bytes != INVALID && free_bytes >= request;
        let descriptor_can_fit =
            free_head != INVALID && free_descriptor_count > 0 && free_descriptor_chain_ok;
        let gap_can_fit = largest_gap >= request;

        tracing::debug!("[PHASE8_1] Inotia2 dereferenced heap-state scan active");
        tracing::debug!(
            "[INOTIA2_HEAP] descriptor_limit={descriptor_limit:#010x} capacity={capacity:#010x} used={used:#010x} free={free_bytes:#010x} heap_base={heap_base:#010x} heap_source={heap_source:#010x} block_table={block_table:#010x} free_head={free_head:#010x} alloc_head={alloc_head:#010x} alloc_count={alloc_count:#010x} ui_ptr={ui_ptr:#010x}"
        );
        tracing::debug!(
            "[INOTIA2_HEAP] ptrs descriptor_limit={descriptor_limit_ptr:#010x} capacity={capacity_ptr:#010x} heap_base={heap_base_ptr:#010x} free_head={free_head_ptr:#010x} used={used_ptr:#010x} alloc_head={alloc_head_ptr:#010x} alloc_count={alloc_count_ptr:#010x} ui_global={ui_ptr_global:#010x}"
        );
        tracing::debug!(
            "[INOTIA2_HEAP] chains free_desc_count={free_descriptor_count} free_chain_ok={free_descriptor_chain_ok} allocated_chain_count={allocated_chain_count} allocated_chain_ok={allocated_chain_ok} alloc_size_sum={allocated_size_sum} monotonic={monotonic} largest_gap={largest_gap:#010x} request_0x100 capacity_can_fit={capacity_can_fit} descriptor_can_fit={descriptor_can_fit} gap_can_fit={gap_can_fit} sample={sample:?}"
        );

        if let Some(regs) = context.debug_cpu_context() {
            // Phase 8.2: the call site in client.bin1149832 is now identified.
            //
            // Guest 0x12300c dispatches MC_dbListDataBase and returns at
            // 0x12301f.  Its caller, 0x1450bc, keeps a resource/storage
            // requirement in callee-saved r7, then immediately compares the
            // value returned by MC_dbListDataBase against:
            //
            //     required = r7 + 0x2800
            //
            // before deciding whether startup can continue.
            //
            // Because r7 is callee-saved it is still live while the WIPI SVC
            // is executing, so we can observe the exact threshold without
            // modifying guest state.
            let required_storage = (regs[7] as u64).saturating_add(0x2800);
            let available_storage = available as i64;
            let storage_margin = available_storage - required_storage as i64;
            let storage_check_pass =
                available_storage >= 0 && (available_storage as u64) >= required_storage;

            tracing::debug!(
                "[PHASE8_2] Inotia2 startup storage-gate trace active"
            );
            tracing::debug!(
                "[INOTIA2_STORAGE_GATE] available={available_storage} r7_resource_total={:#010x} required=r7+0x2800={required_storage} margin={storage_margin} would_pass={storage_check_pass} caller_lr={:#010x} svc_pc={:#010x}",
                regs[7],
                regs[14],
                regs[15]
            );
            tracing::debug!(
                "[INOTIA2_STORAGE_GATE] regs r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x} r4={:#010x} r5={:#010x} r6={:#010x} r7={:#010x} r8={:#010x} r9={:#010x} r10={:#010x} r11={:#010x} r12={:#010x} sp={:#010x} lr={:#010x} pc={:#010x} cpsr={:#010x}",
                regs[0], regs[1], regs[2], regs[3],
                regs[4], regs[5], regs[6], regs[7],
                regs[8], regs[9], regs[10], regs[11], regs[12],
                regs[13], regs[14], regs[15], regs[16]
            );
        }

        let alloc_sig0: u32 =
            read_generic(context, 0x0012_5c54).unwrap_or(INVALID);
        let alloc_sig1: u32 =
            read_generic(context, 0x0012_5c58).unwrap_or(INVALID);
        let init_sig0: u32 =
            read_generic(context, 0x0014_50bc).unwrap_or(INVALID);
        let init_sig1: u32 =
            read_generic(context, 0x0014_50c0).unwrap_or(INVALID);
        tracing::debug!(
            "[INOTIA2_HEAP] signatures alloc@125c54=[{alloc_sig0:#010x},{alloc_sig1:#010x}] init@1450bc=[{init_sig0:#010x},{init_sig1:#010x}]"
        );

        // Phase 8.3.1 — guaranteed i_pack pre-rebuild checkpoint.
        //
        // Static analysis of client.bin1149832 now gives the exact sequence:
        //
        //   0x1450bc succeeds
        //   0x17812e -> 0x144f48
        //   0x144f48 -> 0x1439ac
        //
        // and 0x1439ac performs an internal allocation *before* it calls the
        // database open routine that Phase 8.3 traced.  Therefore, if this
        // pre-open allocation fails, none of the [INOTIA2_DB] lines can ever
        // appear even though the Phase 8.3 code is present.
        //
        // The allocation size is:
        //
        //   request = (u16_global - 1) * 4
        //
        // where the global is reached through GOT[0x01c0].
        //
        // We also inspect the globals populated by 0x143a88 when it parses the
        // packaged i_pack.dat header.  This is observational only.
        let rebuild_count_ptr: u32 =
            read_generic(context, GOT_BASE + 0x01c0).unwrap_or(INVALID);
        let rebuild_count: u16 =
            read_generic(context, rebuild_count_ptr).unwrap_or(0xffff);
        let rebuild_request = if rebuild_count != 0xffff {
            (rebuild_count as u32).wrapping_sub(1).wrapping_mul(4)
        } else {
            INVALID
        };

        // 0x143a88 i_pack.dat header globals.
        let ipack_version_ptr: u32 =
            read_generic(context, GOT_BASE + 0x1610).unwrap_or(INVALID);
        let ipack_count_ptr: u32 =
            read_generic(context, GOT_BASE + 0x1614).unwrap_or(INVALID);
        let ipack_handle_ptr: u32 =
            read_generic(context, GOT_BASE + 0x1608).unwrap_or(INVALID);
        let ipack_array_ptr: u32 =
            read_generic(context, GOT_BASE + 0x160c).unwrap_or(INVALID);

        let ipack_version: u8 =
            read_generic(context, ipack_version_ptr).unwrap_or(0xff);
        let ipack_count: u16 =
            read_generic(context, ipack_count_ptr).unwrap_or(0xffff);
        let ipack_handle: u32 =
            read_generic(context, ipack_handle_ptr).unwrap_or(INVALID);
        let ipack_array: u32 =
            read_generic(context, ipack_array_ptr).unwrap_or(INVALID);

        let packaged_ipack = read_packaged_database(context, "i_pack.dat").await;
        match packaged_ipack {
            Ok(Some(data)) => {
                let packaged_version = data.first().copied().unwrap_or(0xff);
                let packaged_count = if data.len() >= 3 {
                    u16::from_le_bytes([data[1], data[2]])
                } else {
                    0xffff
                };
                let head_len = data.len().min(16);

                tracing::debug!("[PHASE8_3_1] Inotia2 i_pack pre-rebuild checkpoint active");
                tracing::debug!(
                    "[INOTIA2_IPACK_PRE] packaged_found=true packaged_len={} packaged_version={} packaged_count={} head={:02x?}",
                    data.len(),
                    packaged_version,
                    packaged_count,
                    &data[..head_len]
                );
            }
            Ok(None) => {
                tracing::debug!("[PHASE8_3_1] Inotia2 i_pack pre-rebuild checkpoint active");
                tracing::debug!(
                    "[INOTIA2_IPACK_PRE] packaged_found=false packaged_len=0"
                );
            }
            Err(err) => {
                tracing::debug!("[PHASE8_3_1] Inotia2 i_pack pre-rebuild checkpoint active");
                tracing::debug!(
                    "[INOTIA2_IPACK_PRE] packaged_lookup_error={err:?}"
                );
            }
        }

        tracing::debug!(
            "[INOTIA2_IPACK_PRE] rebuild_count_ptr={rebuild_count_ptr:#010x} rebuild_count={rebuild_count} rebuild_request={rebuild_request:#010x} free_bytes={free_bytes:#010x} request_fits={} ipack_version_ptr={ipack_version_ptr:#010x} ipack_version={ipack_version} ipack_count_ptr={ipack_count_ptr:#010x} ipack_count={ipack_count} ipack_handle_ptr={ipack_handle_ptr:#010x} ipack_handle={ipack_handle:#010x} ipack_array_ptr={ipack_array_ptr:#010x} ipack_array={ipack_array:#010x}",
            rebuild_request != INVALID && rebuild_request <= free_bytes
        );
    }

    Ok(available)
}

pub async fn seek_record_single(context: &mut dyn WIPICContext, db_id: i32, offset: i32, origin: i32) -> Result<i32> {
    tracing::debug!("MC_dbSeekRecordSingle({db_id:#x}, {offset}, {origin})");

    let Some(mut handle) = load_handle(context, db_id)? else {
        return Ok(-25); // M_E_INVALIDHANDLE
    };

    let base = match origin {
        0 => 0,
        1 => handle.read_cursor as i64,
        2 => handle.buffer_len as i64,
        _ => return Ok(-1),
    };
    let position = (base + offset as i64).clamp(0, handle.buffer_len as i64) as u32;
    if context.system().pid() == "PD005362" {
        tracing::debug!(
            "[INOTIA1_SAVE] SEEK db={} offset={offset} origin={origin} old_read={} old_write={} -> {position} len={}",
            handle_name(&handle),
            handle.read_cursor,
            handle.write_cursor,
            handle.buffer_len
        );
    }
    handle.read_cursor = position;
    handle.write_cursor = position;
    write_generic(context, db_id as _, handle)?;

    Ok(position as i32)
}

pub async fn list_record_info(context: &mut dyn WIPICContext, ptr_name: WIPICWord, buf_ptr: WIPICWord, capacity: WIPICWord) -> Result<i32> {
    tracing::debug!("MC_dbListRecordInfo({ptr_name:#x}, {buf_ptr:#x}, {capacity})");

    let Ok(name) = String::from_utf8(read_null_terminated_string_bytes(context, ptr_name)?) else {
        return Ok(-22);
    };
    let system = context.system();
    let pid = system.pid().to_owned();

    if !system.platform().database_repository().exists(&name, &pid).await {
        if let Some(data) = read_packaged_database(context, &name).await? {
            if capacity > 0 {
                write_generic(context, buf_ptr, 1u32)?;
                write_generic(context, buf_ptr + 4, 0u32)?;
                write_generic(context, buf_ptr + 8, data.len() as u32)?;
            }
            if pid == "PD005362" {
                tracing::info!(
                    "[INOTIA1_META] LIST_RECORD_INFO db={name} source=packaged capacity={capacity} record_id=1 record_size={}",
                    data.len()
                );
            }
            return Ok(0);
        }
        if pid == "PD005362" {
            tracing::info!(
                "[INOTIA1_META] LIST_RECORD_INFO db={name} capacity={capacity} -> NOENT"
            );
        }
        return Ok(-12); // M_E_NOENT
    }

    let db = system.platform().database_repository().open(&name, &pid).await;
    let ids = db.get_record_ids().await;

    let mut written = 0;
    for id in ids {
        if written >= capacity {
            break;
        }

        let Some(data) = db.get(id).await else {
            continue;
        };

        let entry_ptr = buf_ptr + written * 12;
        write_generic(context, entry_ptr, id)?;
        write_generic(context, entry_ptr + 4, 0u32)?;
        write_generic(context, entry_ptr + 8, data.len() as u32)?;
        if pid == "PD005362" {
            tracing::info!(
                "[INOTIA1_META] LIST_RECORD_INFO db={name} capacity={capacity} entry={written} record_id={id} record_size={}",
                data.len()
            );
        }
        written += 1;
    }

    if pid == "PD005362" {
        tracing::info!(
            "[INOTIA1_META] LIST_RECORD_INFO db={name} entries_written={written} -> 0"
        );
    }

    Ok(0)
}

pub async fn exists_database(context: &mut dyn WIPICContext, ptr_name: WIPICWord, r#type: i32) -> Result<i32> {
    tracing::debug!("MC_dbExistsDataBase({ptr_name:#x}, {type})");

    let Ok(name) = String::from_utf8(read_null_terminated_string_bytes(context, ptr_name)?) else {
        return Ok(-22);
    };
    let (pid, aid) = {
        let system = context.system();
        (system.pid().to_owned(), system.aid().to_owned())
    };
    if pid == "PD109653" && aid == "000262F4" {
        let key = bm3_host_db_cache_key(&name);
        if context.host_blob_cache_get(&key).is_some() || read_packaged_database(context, &name).await?.is_some() {
            tracing::info!("[PHASE8_82_BM3_DB_EXISTS] name={name:?} result=0 source=session_or_packaged");
            return Ok(0);
        }
        tracing::info!("[PHASE8_82_BM3_DB_EXISTS] name={name:?} result=-12 source=session_memory");
        return Ok(-12);
    }
    if read_packaged_database(context, &name).await?.is_some() {
        if context.system().pid() == "PD005362" {
            tracing::info!("[INOTIA1_META] EXISTS_STANDARD db={name} source=packaged -> 0");
        }
        return Ok(0);
    }

    let system = context.system();
    let exists = system.platform().database_repository().exists(&name, &pid).await;
    let result = if exists { 0 } else { -12 };
    if pid == "PD005362" {
        tracing::info!(
            "[INOTIA1_META] EXISTS_STANDARD db={name} exists={exists} -> {result}"
        );
    }
    Ok(result)
}

pub async fn stream_write(context: &mut dyn WIPICContext, db_id: i32, buf_ptr: WIPICWord, buf_len: WIPICWord) -> Result<i32> {
    tracing::debug!("db.stream_write({db_id:#x}, {buf_ptr:#x}, {buf_len})");

    let Some(mut handle) = load_handle(context, db_id)? else {
        return Ok(-25); // M_E_INVALIDHANDLE
    };

    // Cursor + len is guest-controlled, so guard the arithmetic. An
    // overflowed `new_end` would silently bypass the capacity check below
    // and let a write spill into unrelated guest memory.
    let Some(new_end) = handle.write_cursor.checked_add(buf_len) else {
        return Ok(-22); // M_E_BADRECID — closest "bad parameter" code
    };

    let old_len = handle.buffer_len;
    let db_name = handle_name(&handle).to_owned();
    let (pid, aid) = {
        let system = context.system();
        (system.pid().to_owned(), system.aid().to_owned())
    };

    if pid == "PD005362" {
        if db_name.starts_with("save") {
            tracing::info!(
                "[INOTIA1_SAVE_COMMIT] WRITE begin db={db_name} offset={} len={buf_len} old_len={old_len}",
                handle.write_cursor
            );
        } else {
            tracing::debug!(
                "[INOTIA1_SAVE] WRITE begin db={db_name} offset={} len={buf_len} old_len={old_len}",
                handle.write_cursor
            );
        }
    }
    if pid == "PD007974" {
        let mut head = vec![0u8; (buf_len as usize).min(16)];
        let head_read = if head.is_empty() {
            0
        } else {
            context.read_bytes(buf_ptr, &mut head).unwrap_or(0)
        };
        let cpu = context.debug_cpu_context();
        if let Some(regs) = cpu {
            tracing::debug!(
                "[INOTIA2_DB] WRITE_BEGIN db={db_name} handle={db_id:#010x} offset={} request={buf_len} old_len={old_len} head_read={head_read} head={:02x?} lr={:#010x} pc={:#010x}",
                handle.write_cursor,
                &head[..head_read.min(head.len())],
                regs[14],
                regs[15]
            );
        } else {
            tracing::debug!(
                "[INOTIA2_DB] WRITE_BEGIN db={db_name} handle={db_id:#010x} offset={} request={buf_len} old_len={old_len} head_read={head_read} head={:02x?}",
                handle.write_cursor,
                &head[..head_read.min(head.len())]
            );
        }
    }

    // Grow the guest-heap buffer if the next write would land past its
    // end. Doubling-on-demand starting from MIN_BUFFER_CAPACITY keeps the
    // realloc count amortized; alloc/free is a guest-side `WIPICContext`
    // primitive so we copy old bytes via host-side scratch.
    if new_end > handle.buffer_capacity {
        let Some(rounded) = new_end.checked_next_power_of_two() else {
            return Ok(-22);
        };
        let new_cap = rounded.max(MIN_BUFFER_CAPACITY);
        let new_ptr = context.alloc_raw(new_cap)?;
        if handle.buffer_len > 0 && handle.buffer_ptr != 0 {
            let mut old_data = vec![0u8; handle.buffer_len as usize];
            context.read_bytes(handle.buffer_ptr, &mut old_data)?;
            context.write_bytes(new_ptr, &old_data)?;
        }
        if handle.buffer_ptr != 0 && handle.buffer_capacity > 0 {
            context.free_raw(handle.buffer_ptr, handle.buffer_capacity)?;
        }
        handle.buffer_ptr = new_ptr;
        handle.buffer_capacity = new_cap;
    }

    // If the write_cursor was seeked past the prior end (e.g. via a slot 4
    // multi-slot save), the bytes between the old end and the cursor were
    // never initialised. `alloc_raw` doesn't guarantee zeroed memory and
    // the snapshot below is flushed straight to disk, so explicitly zero
    // the gap to avoid leaking heap residue into the save file. This must
    // run for `buf_len == 0` too: `new_end == write_cursor` still extends
    // `buffer_len`, so the gap would otherwise be snapshotted uninitialised.
    if handle.write_cursor > old_len {
        let gap_size = (handle.write_cursor - old_len) as usize;
        let zeros = vec![0u8; gap_size];
        context.write_bytes(handle.buffer_ptr + old_len, &zeros)?;
    }

    if buf_len > 0 {
        let mut buf = vec![0u8; buf_len as usize];
        context.read_bytes(buf_ptr, &mut buf)?;
        context.write_bytes(handle.buffer_ptr + handle.write_cursor, &buf)?;
    }

    handle.write_cursor = new_end;
    if new_end > handle.buffer_len {
        handle.buffer_len = new_end;
    }
    write_generic(context, db_id as _, handle)?;

    // Phase 8.18 — the exact Inotia 2 static install records are intentionally
    // write-back cached. Avoid both repository I/O and the O(n) full-buffer
    // snapshot on every small append. close_database performs one guarded
    // canonical commit after the native rebuild has initialized its tables.
    if pid == "PD007974"
        && handle.open_mode == 4
        && is_inotia2_static_install_resource(&db_name)
    {
        tracing::debug!(
            "[PHASE8_18_INOTIA2_INSTALL_WRITEBACK] WRITE_DEFER name={db_name} offset={} len={buf_len} final_len={}",
            handle.write_cursor.saturating_sub(buf_len),
            handle.buffer_len
        );
        return Ok(buf_len as _);
    }

    // Write-through to disk on every stream_write. Some titles tear down
    // the game without making a final `close_database` call after their
    // save sequence — relying on close as the only flush point loses all
    // the writes that landed since the session opened. Flushing eagerly
    // costs an extra small file write per call but keeps the on-disk state
    // consistent if the process exits or the title forgets to close.
    let mut snapshot = vec![0u8; handle.buffer_len as usize];
    if handle.buffer_ptr != 0 && handle.buffer_len > 0 {
        context.read_bytes(handle.buffer_ptr, &mut snapshot)?;
    }

    // Phase 8.19 — persist envinfo exactly as the game writes it. Shadow,
    // weather, and critical effects are no longer forcibly disabled.

    if let Some(mut db) = open_db_for_handle(context, &handle).await {
        db.set(1, &snapshot).await;
    }

    if pid == "PD007974" && aid == "010100D5" && is_inotia2_static_install_resource(&db_name) {
        let key = inotia2_host_db_cache_key(&db_name);
        context.host_blob_cache_put(&key, snapshot.clone());
        tracing::debug!(
            "[PHASE8_21_INOTIA2_DB_CACHE] STORE name={db_name} len={} source=write-through",
            snapshot.len()
        );
    }

    if pid == "PD007974" {
        tracing::debug!(
            "[INOTIA2_DB] WRITE_RESULT db={db_name} handle={db_id:#010x} wrote={buf_len} final_len={} read_cursor={} write_cursor={}",
            handle.buffer_len,
            handle.read_cursor,
            handle.write_cursor
        );
    }

    if pid == "PD005362" {
        if db_name.starts_with("save") {
            let fp = inotia_fingerprint(&snapshot);
            let first320_len = snapshot.len().min(320);
            let first320_fp = inotia_fingerprint(&snapshot[..first320_len]);
            tracing::info!(
                "[INOTIA1_SAVE_COMMIT] WRITE commit db={db_name} final_len={} cursor={} fnv64={fp:016x} first320_len={first320_len} first320_fnv64={first320_fp:016x}",
                snapshot.len(),
                handle.write_cursor
            );
        } else {
            tracing::debug!(
                "[INOTIA1_SAVE] WRITE commit db={db_name} final_len={} cursor={}",
                snapshot.len(),
                handle.write_cursor
            );
        }
    }

    Ok(buf_len as _)
}

/// Standard WIPI `MC_dbDeleteRecord(handle, rec_id)` — delete a single
/// record by id from an open DB handle.
pub async fn delete_record(context: &mut dyn WIPICContext, db_id: i32, rec_id: i32) -> Result<i32> {
    tracing::debug!("MC_dbDeleteRecord({db_id:#x}, {rec_id})");

    let Some(handle) = load_handle(context, db_id)? else {
        return Ok(-25); // M_E_INVALIDHANDLE
    };
    let Some(mut db) = open_db_for_handle(context, &handle).await else {
        return Ok(-25);
    };
    if context.system().pid() == "PD005362" {
        tracing::debug!(
            "[INOTIA1_SAVE] DELETE_RECORD db={} rec_id={rec_id}",
            handle_name(&handle)
        );
    }
    let ok = db.delete(rec_id as u32).await;
    Ok(if ok { 0 } else { -22 })
}

/// KTF reuses slot 6 with two call shapes that share the same SVC signature:
///
///  - standard WIPI: `delete_record(handle, rec_id)`
///  - KTF custom:    `(name_ptr, type)` — used as a name-keyed cleanup
///
/// Both pass two ints, so we disambiguate by reading the magic field at
/// `a0`. A real handle starts with `DATABASE_HANDLE_MAGIC`; a name pointer
/// (or anything else) does not, and we fall back to a no-op.
pub async fn delete_record_ktf(context: &mut dyn WIPICContext, a0: i32, a1: i32) -> Result<i32> {
    if load_handle(context, a0)?.is_some() {
        return delete_record(context, a0, a1).await;
    }

    // Not a real handle — KTF name-keyed form. No-op preserves saves; the
    // bytes of a name string would otherwise round-trip into the standard
    // path and silently delete record 1 of the just-saved DB.
    tracing::debug!("MC_dbDeleteRecord(name-keyed @ {a0:#x}, {a1}) -> 0 (no-op)");
    Ok(0)
}

pub async fn delete_database(context: &mut dyn WIPICContext, ptr_name: WIPICWord, flags: i32) -> Result<i32> {
    tracing::debug!("MC_dbDeleteDataBase({ptr_name:#x}, {flags})");

    let Ok(name) = String::from_utf8(read_null_terminated_string_bytes(context, ptr_name)?) else {
        return Ok(-22);
    };
    let system = context.system();
    let pid = system.pid().to_owned();

    let deleted = system.platform().database_repository().delete(&name, &pid).await;
    if deleted || !system.platform().database_repository().exists(&name, &pid).await {
        Ok(0)
    } else {
        Ok(-12) // M_E_NOENT
    }
}

pub async fn update_record(context: &mut dyn WIPICContext, db_id: i32, rec_id: i32, buf_ptr: WIPICWord, buf_len: WIPICWord) -> Result<i32> {
    tracing::debug!("MC_dbUpdateRecord({db_id:#x}, {rec_id}, {buf_ptr:#x}, {buf_len})");

    let Some(handle) = load_handle(context, db_id)? else {
        return Ok(-25); // M_E_INVALIDHANDLE
    };
    let Some(mut db) = open_db_for_handle(context, &handle).await else {
        return Ok(-25);
    };
    if rec_id < 0 {
        return Ok(-22);
    }
    let rec_id = rec_id as u32;
    if db.get(rec_id).await.is_none() {
        return Ok(-22);
    }

    let mut buf = vec![0; buf_len as usize];
    context.read_bytes(buf_ptr, &mut buf)?;

    if db.set(rec_id, &buf).await { Ok(0) } else { Ok(-22) }
}

pub async fn select_record(context: &mut dyn WIPICContext, db_id: i32, rec_id: i32, buf_ptr: WIPICWord, buf_len: WIPICWord) -> Result<i32> {
    tracing::debug!("MC_dbSelectRecord({db_id:#x}, {rec_id}, {buf_ptr:#x}, {buf_len})");

    let Some(handle) = load_handle(context, db_id)? else {
        return Ok(-25); // M_E_INVALIDHANDLE
    };
    let Some(db) = open_db_for_handle(context, &handle).await else {
        return Ok(-25);
    };
    if rec_id < 0 {
        return Ok(-22);
    }

    if let Some(data) = db.get(rec_id as u32).await {
        if buf_len < data.len() as u32 {
            return Ok(-18); // M_E_SHORTBUF
        }
        context.write_bytes(buf_ptr, &data)?;
        Ok(0)
    } else {
        Ok(-22)
    }
}

pub async fn stream_read(context: &mut dyn WIPICContext, db_id: i32, buf_ptr: WIPICWord, buf_len: WIPICWord) -> Result<i32> {
    tracing::debug!("db.stream_read({db_id:#x}, {buf_ptr:#x}, {buf_len})");

    let Some(mut handle) = load_handle(context, db_id)? else {
        return Ok(-25); // M_E_INVALIDHANDLE
    };

    let inotia2_db_trace = context.system().pid() == "PD007974";
    if inotia2_db_trace {
        let cpu = context.debug_cpu_context();
        if let Some(regs) = cpu {
            tracing::debug!(
                "[INOTIA2_DB] READ_BEGIN db={} handle={db_id:#010x} offset={} request={buf_len} record_len={} lr={:#010x} pc={:#010x}",
                handle_name(&handle),
                handle.read_cursor,
                handle.buffer_len,
                regs[14],
                regs[15]
            );
        } else {
            tracing::debug!(
                "[INOTIA2_DB] READ_BEGIN db={} handle={db_id:#010x} offset={} request={buf_len} record_len={}",
                handle_name(&handle),
                handle.read_cursor,
                handle.buffer_len
            );
        }
    }

    if handle.read_cursor >= handle.buffer_len {
        if inotia2_db_trace {
            tracing::debug!(
                "[INOTIA2_DB] READ_RESULT db={} handle={db_id:#010x} -> -23 (EOF) offset={} record_len={}",
                handle_name(&handle),
                handle.read_cursor,
                handle.buffer_len
            );
        }
        // Don't touch buf — caller may have passed a sentinel (NULL) that
        // we shouldn't write to. Some titles do this past EOF.
        return Ok(-23); // M_E_EOF
    }

    // Phase 8.1.2 defensive invariant: after the KTF slot-4 length query,
    // Inotia 1 should request the complete save0 record.  Do not mutate the
    // request here (the guest owns its buffer size); emit a loud diagnostic
    // if another code path ever causes the lengths to diverge again.
    if context.system().pid() == "PD005362"
        && handle_name(&handle) == "save0.dat"
        && handle.read_cursor == 0
        && handle.buffer_len > 0
        && buf_len != handle.buffer_len
    {
        tracing::warn!(
            "[INOTIA1_LENGTH_MISMATCH] request={buf_len} record_len={} delta={} -- Continue may reject this slot",
            handle.buffer_len,
            handle.buffer_len as i64 - buf_len as i64
        );
    }

    let take = core::cmp::min(buf_len, handle.buffer_len - handle.read_cursor);
    if take == 0 {
        return Ok(0);
    }

    // Copy from the guest-heap buffer into the caller's destination via
    // host-side scratch; `WIPICContext` doesn't expose an in-guest memmove.
    let mut data = vec![0u8; take as usize];
    context.read_bytes(handle.buffer_ptr + handle.read_cursor, &mut data)?;
    context.write_bytes(buf_ptr, &data)?;

    let old_cursor = handle.read_cursor;
    handle.read_cursor += take;
    write_generic(context, db_id as _, handle)?;

    if context.system().pid() == "PD005362" && handle_name(&handle).starts_with("save") {
        // Phase 8.42: only save-slot reads retain INFO diagnostics. Resource
        // databases (char/map/tile/mon/pattern/etc.) are hot-path data and no
        // longer pay fingerprint/format/log costs on every stream read.
        // `data` is exactly what the guest received.  For save files, also
        // fingerprint the complete backing record so a 320-byte Continue
        // read can be correlated with a 324-byte persisted generation without
        // changing the bytes or cursor semantics.
        let returned = &data[..];
        let fp = inotia_fingerprint(returned);
        let remaining = handle.buffer_len.saturating_sub(handle.read_cursor);

        if handle_name(&handle).starts_with("save") {
            let mut backing = vec![0u8; handle.buffer_len as usize];
            if handle.buffer_ptr != 0 && handle.buffer_len > 0 {
                context.read_bytes(handle.buffer_ptr, &mut backing)?;
            }
            let backing_fp = inotia_fingerprint(&backing);
            let first320_len = backing.len().min(320);
            let first320_fp = inotia_fingerprint(&backing[..first320_len]);
            tracing::info!(
                "[INOTIA1_SAVE_READ] db={} offset={} request={buf_len} returned={take} final_cursor={} record_len={} remaining={remaining} returned_fnv64={fp:016x} backing_fnv64={backing_fp:016x} backing_first320_len={first320_len} backing_first320_fnv64={first320_fp:016x}",
                handle_name(&handle),
                old_cursor,
                handle.read_cursor,
                handle.buffer_len
            );

            // Legacy Phase 7.19 CPU/frame snapshots were removed in Phase 8.42.
            // Phase 7.21 save-length behavior is already field-validated, and
            // those stack/code reads are unnecessary on the gameplay path.
        }
    }

    if inotia2_db_trace {
        let head_end = data.len().min(16);
        tracing::debug!(
            "[INOTIA2_DB] READ_RESULT db={} handle={db_id:#010x} returned={take} final_cursor={} record_len={} remaining={} head={:02x?}",
            handle_name(&handle),
            handle.read_cursor,
            handle.buffer_len,
            handle.buffer_len.saturating_sub(handle.read_cursor),
            &data[..head_end]
        );

        // Phase 8.5 — KTF Inotia 2 i_pack post-header global-pointer probe.
        //
        // Phase 8.4 now exposes p/i_pack.dat correctly.  The guest parses its
        // 55-byte header (version + count + 13 u32 offsets) successfully, then
        // deterministically faults at address 0 before another WIPI call.
        // Static disassembly places the next guest instructions at 0x143adc,
        // where four PIC/GOT-resolved globals are written.  Capture those GOT
        // entries and their destinations while we are still inside the final
        // stream-read SVC, immediately before control returns to the guest.
        // This block is observational only.
        if handle_name(&handle) == "i_pack.dat"
            && old_cursor == 51
            && take == 4
            && handle.read_cursor == 55
        {
            const INVALID: u32 = 0xffff_ffff;

            // r8 in 0x143a88 resolves to 0x1924c4.  The literals used by the
            // post-header stores are offsets 0x1610, 0x1614, 0x1608, 0x160c.
            const GOT_VERSION: u32 = 0x0019_3ad4;
            const GOT_COUNT: u32 = 0x0019_3ad8;
            const GOT_HANDLE: u32 = 0x0019_3acc;
            const GOT_ARRAY: u32 = 0x0019_3ad0;

            let version_target: u32 = read_generic(context, GOT_VERSION).unwrap_or(INVALID);
            let count_target: u32 = read_generic(context, GOT_COUNT).unwrap_or(INVALID);
            let handle_target: u32 = read_generic(context, GOT_HANDLE).unwrap_or(INVALID);
            let array_target: u32 = read_generic(context, GOT_ARRAY).unwrap_or(INVALID);

            let version_probe: u32 = if version_target != 0 && version_target != INVALID {
                read_generic(context, version_target).unwrap_or(INVALID)
            } else { INVALID };
            let count_probe: u32 = if count_target != 0 && count_target != INVALID {
                read_generic(context, count_target).unwrap_or(INVALID)
            } else { INVALID };
            let handle_probe: u32 = if handle_target != 0 && handle_target != INVALID {
                read_generic(context, handle_target).unwrap_or(INVALID)
            } else { INVALID };
            let array_probe: u32 = if array_target != 0 && array_target != INVALID {
                read_generic(context, array_target).unwrap_or(INVALID)
            } else { INVALID };

            let code0: u32 = read_generic(context, 0x0014_3adc).unwrap_or(INVALID);
            let code1: u32 = read_generic(context, 0x0014_3ae0).unwrap_or(INVALID);
            let code2: u32 = read_generic(context, 0x0014_3ae4).unwrap_or(INVALID);
            let code3: u32 = read_generic(context, 0x0014_3ae8).unwrap_or(INVALID);
            let code4: u32 = read_generic(context, 0x0014_3aec).unwrap_or(INVALID);
            let code5: u32 = read_generic(context, 0x0014_3af0).unwrap_or(INVALID);
            let code6: u32 = read_generic(context, 0x0014_3af4).unwrap_or(INVALID);
            let code7: u32 = read_generic(context, 0x0014_3af8).unwrap_or(INVALID);

            tracing::debug!("[PHASE8_5] Inotia2 i_pack post-header global-pointer probe active");
            tracing::debug!(
                "[INOTIA2_IPACK_POST] got version@{GOT_VERSION:#010x}->{version_target:#010x} probe={version_probe:#010x} count@{GOT_COUNT:#010x}->{count_target:#010x} probe={count_probe:#010x} handle@{GOT_HANDLE:#010x}->{handle_target:#010x} probe={handle_probe:#010x} array@{GOT_ARRAY:#010x}->{array_target:#010x} probe={array_probe:#010x}"
            );
            tracing::debug!(
                "[INOTIA2_IPACK_POST] code@143adc=[{code0:#010x},{code1:#010x},{code2:#010x},{code3:#010x},{code4:#010x},{code5:#010x},{code6:#010x},{code7:#010x}]"
            );

            // Phase 8.6 — probe the *next* caller's validation globals.
            //
            // Phase 8.5 proved that all four globals written by 0x143adc are
            // correctly relocated and mapped. The null fault therefore occurs
            // after 0x143a88 returns to 0x144e58.
            //
            // Static disassembly of 0x144e58 shows three GOT-resolved globals
            // are dereferenced immediately:
            //
            //   GOT+0x03b8 -> u16 validation count
            //   GOT+0x03bc -> u8  record stride
            //   GOT+0x03b4 -> u32 record-base pointer
            //
            // A NULL GOT destination here exactly matches the observed
            // "Invalid memory access; address: 0". Capture both the GOT
            // destinations and the values they currently contain.
            const GOT_VALIDATE_BASE: u32 = 0x0019_2878;   // GOT base + 0x03b4
            const GOT_VALIDATE_COUNT: u32 = 0x0019_287c;  // GOT base + 0x03b8
            const GOT_VALIDATE_STRIDE: u32 = 0x0019_2880; // GOT base + 0x03bc

            let validate_base_target: u32 =
                read_generic(context, GOT_VALIDATE_BASE).unwrap_or(INVALID);
            let validate_count_target: u32 =
                read_generic(context, GOT_VALIDATE_COUNT).unwrap_or(INVALID);
            let validate_stride_target: u32 =
                read_generic(context, GOT_VALIDATE_STRIDE).unwrap_or(INVALID);

            let validate_base: u32 =
                if validate_base_target != 0 && validate_base_target != INVALID {
                    read_generic(context, validate_base_target).unwrap_or(INVALID)
                } else {
                    INVALID
                };
            let validate_count: u16 =
                if validate_count_target != 0 && validate_count_target != INVALID {
                    read_generic(context, validate_count_target).unwrap_or(0xffff)
                } else {
                    0xffff
                };
            let validate_stride: u8 =
                if validate_stride_target != 0 && validate_stride_target != INVALID {
                    read_generic(context, validate_stride_target).unwrap_or(0xff)
                } else {
                    0xff
                };

            let first_record_addr = if validate_base != 0 && validate_base != INVALID {
                validate_base.saturating_add(6)
            } else {
                validate_base
            };

            let mut first_record = [0u8; 32];
            let first_record_read =
                if first_record_addr != 0 && first_record_addr != INVALID {
                    context.read_bytes(first_record_addr, &mut first_record).unwrap_or(0)
                } else {
                    0
                };

            let null_stage = if validate_count_target == 0 {
                "count_got_target_null"
            } else if validate_count != 0 && validate_stride_target == 0 {
                "stride_got_target_null"
            } else if validate_count != 0 && validate_base_target == 0 {
                "base_got_target_null"
            } else if validate_count != 0 && validate_base == 0 {
                "record_base_value_null"
            } else {
                "none_obvious"
            };

            tracing::debug!(
                "[PHASE8_6] Inotia2 post-i_pack caller validation-global probe active"
            );
            tracing::debug!(
                "[INOTIA2_VALIDATE_POST] got base@{GOT_VALIDATE_BASE:#010x}->{validate_base_target:#010x} value={validate_base:#010x} count@{GOT_VALIDATE_COUNT:#010x}->{validate_count_target:#010x} value={validate_count} stride@{GOT_VALIDATE_STRIDE:#010x}->{validate_stride_target:#010x} value={validate_stride} first_record_addr={first_record_addr:#010x} first_record_read={first_record_read} first_record={:02x?} null_stage={null_stage}",
                &first_record[..first_record_read.min(first_record.len())]
            );

            // Signature of the immediate caller block after 0x143a88 returns.
            let caller0: u32 = read_generic(context, 0x0014_4e6a).unwrap_or(INVALID);
            let caller1: u32 = read_generic(context, 0x0014_4e6e).unwrap_or(INVALID);
            let caller2: u32 = read_generic(context, 0x0014_4e72).unwrap_or(INVALID);
            let caller3: u32 = read_generic(context, 0x0014_4e76).unwrap_or(INVALID);
            let caller4: u32 = read_generic(context, 0x0014_4e7a).unwrap_or(INVALID);
            let caller5: u32 = read_generic(context, 0x0014_4e7e).unwrap_or(INVALID);
            tracing::debug!(
                "[INOTIA2_VALIDATE_POST] code@144e6a=[{caller0:#010x},{caller1:#010x},{caller2:#010x},{caller3:#010x},{caller4:#010x},{caller5:#010x}]"
            );

            // Phase 8.7 — probe the first dereferences after startup succeeds.
            //
            // Phase 8.6 showed validation_count == 0, so 0x144e58 returns 1
            // without touching validate_base/validate_stride. 0x1450bc then
            // stores that success byte through GOT+0x1638 and returns 1.
            //
            // The outer startup path next calls 0x144f48. If the success byte
            // is non-zero, 0x144f48 immediately returns 1 and execution enters
            // 0x144a2c.
            //
            // 0x144a2c begins with:
            //
            //   kind  = *(u8  *)GOT[0x0338]
            //   base  = *(u32 *)GOT[0x0330]
            //   src   = base + kind * 129
            //   read_u16(src)
            //
            // read_u16() (guest 0x126088) copies two bytes from `src`; if
            // base==0 and kind==0, that becomes an exact read from address 0,
            // matching the observed fault.
            const GOT_STARTUP_KIND: u32 = 0x0019_27fc;   // GOT + 0x0338
            const GOT_STARTUP_BASE: u32 = 0x0019_27f4;   // GOT + 0x0330
            const GOT_STARTUP_STATUS: u32 = 0x0019_3afc; // GOT + 0x1638

            let startup_kind_target: u32 =
                read_generic(context, GOT_STARTUP_KIND).unwrap_or(INVALID);
            let startup_base_target: u32 =
                read_generic(context, GOT_STARTUP_BASE).unwrap_or(INVALID);
            let startup_status_target: u32 =
                read_generic(context, GOT_STARTUP_STATUS).unwrap_or(INVALID);

            let startup_kind: u8 =
                if startup_kind_target != 0 && startup_kind_target != INVALID {
                    read_generic(context, startup_kind_target).unwrap_or(0xff)
                } else {
                    0xff
                };
            let startup_base: u32 =
                if startup_base_target != 0 && startup_base_target != INVALID {
                    read_generic(context, startup_base_target).unwrap_or(INVALID)
                } else {
                    INVALID
                };
            let startup_status: u8 =
                if startup_status_target != 0 && startup_status_target != INVALID {
                    read_generic(context, startup_status_target).unwrap_or(0xff)
                } else {
                    0xff
                };

            let first_src = if startup_base != INVALID && startup_kind != 0xff {
                startup_base.saturating_add((startup_kind as u32).saturating_mul(129))
            } else {
                INVALID
            };

            let mut first_src_bytes = [0u8; 8];
            let first_src_read =
                if first_src != 0 && first_src != INVALID {
                    context.read_bytes(first_src, &mut first_src_bytes).unwrap_or(0)
                } else {
                    0
                };

            // The next four GOT entries in 0x144a2c are direct table pointers.
            let table0: u32 = read_generic(context, 0x0019_3aec).unwrap_or(INVALID);
            let table1: u32 = read_generic(context, 0x0019_3af0).unwrap_or(INVALID);
            let table2: u32 = read_generic(context, 0x0019_3af4).unwrap_or(INVALID);
            let table3: u32 = read_generic(context, 0x0019_3af8).unwrap_or(INVALID);

            let predicted_null_read =
                startup_status != 0
                    && startup_status != 0xff
                    && startup_kind == 0
                    && startup_base == 0
                    && first_src == 0;

            tracing::debug!(
                "[PHASE8_7] Inotia2 post-startup resource-base probe active"
            );
            tracing::debug!(
                "[INOTIA2_STARTUP_RESOURCE] status got@{GOT_STARTUP_STATUS:#010x}->{startup_status_target:#010x} value={startup_status} kind got@{GOT_STARTUP_KIND:#010x}->{startup_kind_target:#010x} value={startup_kind} base got@{GOT_STARTUP_BASE:#010x}->{startup_base_target:#010x} value={startup_base:#010x} first_src={first_src:#010x} first_src_read={first_src_read} first_src_bytes={:02x?} predicted_null_read={predicted_null_read}",
                &first_src_bytes[..first_src_read.min(first_src_bytes.len())]
            );
            tracing::debug!(
                "[INOTIA2_STARTUP_RESOURCE] direct_tables=[{table0:#010x},{table1:#010x},{table2:#010x},{table3:#010x}]"
            );

            // 0x14368c loads game.dat and stores its parsed resource-table
            // source through GOT+0x1604.  0x101950 then calls 0x1432f0
            // repeatedly; resource ID 0x43 writes exactly the `kind` and
            // `base` globals probed above.  This lets the same checkpoint
            // verify the complete causal chain after the seek fix.
            const GOT_GAME_RESOURCE_SOURCE: u32 = 0x0019_3ac8; // GOT+0x1604
            let game_source_target: u32 =
                read_generic(context, GOT_GAME_RESOURCE_SOURCE).unwrap_or(INVALID);
            let game_source: u32 =
                if game_source_target != 0 && game_source_target != INVALID {
                    read_generic(context, game_source_target).unwrap_or(INVALID)
                } else {
                    INVALID
                };

            let mut game_source_head = [0u8; 24];
            let game_source_head_read =
                if game_source != 0 && game_source != INVALID {
                    context.read_bytes(game_source, &mut game_source_head).unwrap_or(0)
                } else {
                    0
                };

            tracing::debug!(
                "[INOTIA2_RESOURCE_INIT] game_source got@{GOT_GAME_RESOURCE_SOURCE:#010x}->{game_source_target:#010x} value={game_source:#010x} head_read={game_source_head_read} head={:02x?} resource43_kind={startup_kind} resource43_base={startup_base:#010x}",
                &game_source_head[..game_source_head_read.min(game_source_head.len())]
            );

            // Signatures covering the success-store and first 0x144a2c read.
            let status0: u32 = read_generic(context, 0x0014_50dc).unwrap_or(INVALID);
            let status1: u32 = read_generic(context, 0x0014_50e0).unwrap_or(INVALID);
            let status2: u32 = read_generic(context, 0x0014_50e4).unwrap_or(INVALID);
            let start0: u32 = read_generic(context, 0x0014_4a44).unwrap_or(INVALID);
            let start1: u32 = read_generic(context, 0x0014_4a48).unwrap_or(INVALID);
            let start2: u32 = read_generic(context, 0x0014_4a4c).unwrap_or(INVALID);
            let start3: u32 = read_generic(context, 0x0014_4a50).unwrap_or(INVALID);
            let start4: u32 = read_generic(context, 0x0014_4a54).unwrap_or(INVALID);
            let start5: u32 = read_generic(context, 0x0014_4a58).unwrap_or(INVALID);
            let start6: u32 = read_generic(context, 0x0014_4a5c).unwrap_or(INVALID);
            tracing::debug!(
                "[INOTIA2_STARTUP_RESOURCE] code status@1450dc=[{status0:#010x},{status1:#010x},{status2:#010x}] start@144a44=[{start0:#010x},{start1:#010x},{start2:#010x},{start3:#010x},{start4:#010x},{start5:#010x},{start6:#010x}]"
            );

            if let Some(regs) = context.debug_cpu_context() {
                let sp = regs[13];
                let mut stack = [0u8; 64];
                let stack_read = context.read_bytes(sp, &mut stack).unwrap_or(0);
                tracing::debug!(
                    "[INOTIA2_IPACK_POST] regs r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x} r4={:#010x} r5={:#010x} r6={:#010x} r7={:#010x} r8={:#010x} r9={:#010x} r10={:#010x} r11={:#010x} r12={:#010x} sp={:#010x} lr={:#010x} pc={:#010x} cpsr={:#010x} stack_read={stack_read} stack={:02x?}",
                    regs[0], regs[1], regs[2], regs[3], regs[4], regs[5], regs[6], regs[7],
                    regs[8], regs[9], regs[10], regs[11], regs[12], regs[13], regs[14], regs[15], regs[16],
                    &stack[..stack_read.min(stack.len())]
                );
            } else {
                tracing::debug!("[INOTIA2_IPACK_POST] CPU snapshot unavailable");
            }
        }
    }

    Ok(take as _)
}

/// KTF custom slot 4 — repurposed from standard `MC_dbSelectRecord` into a
/// stream-control op `(handle, offset, mode)` that seeks both read/write
/// cursors. The standard WIPI signature `(db_id, rec_id, buf_ptr, buf_len)`
/// is not implemented; LGT routes do not use this slot.
pub async fn select_record_ktf(context: &mut dyn WIPICContext, db_id: i32, rec_id: i32, mode: WIPICWord, _buf_len: WIPICWord) -> Result<i32> {
    tracing::debug!("MC_dbSelectRecord({db_id:#x}, {rec_id}, mode={mode:#x}, {_buf_len})");

    let Some(mut handle) = load_handle(context, db_id)? else {
        return Ok(-25); // M_E_INVALIDHANDLE
    };

    // KTF reuses slot 4 as a stream-control op `(handle, offset, mode)`. The
    // shapes observed across games:
    //
    //   - `(handle, slot_offset, 0)` — multi-slot save files store each
    //     slot at a known byte offset within record 1; this seeks both
    //     cursors so the next read/write hits the right slot while
    //     preserving the bytes belonging to the other slots.
    //   - `(handle, 0, 0)` and `(handle, 0, 2)` — rewinds both cursors.
    //     mode=0 vs 2 isn't a length and isn't truncate (truncating on
    //     mode=2 on the read path destroys a prefetched buffer during a
    //     subsequent re-open and wipes the saved record). Both are treated
    //     as plain seek-and-rewind.
    if rec_id >= 0 {
        let offset = rec_id as u32;
        let pid = context.system().pid().to_owned();
        let db_name = handle_name(&handle).to_owned();

        if pid == "PD005362" {
            tracing::debug!(
                "[INOTIA1_SAVE] SELECT/SEEK db={} offset={offset} mode={mode:#x} old_read={} old_write={} len={}",
                db_name,
                handle.read_cursor,
                handle.write_cursor,
                handle.buffer_len
            );
        }
        if pid == "PD007974" {
            let cpu = context.debug_cpu_context();
            if let Some(regs) = cpu {
                tracing::debug!(
                    "[INOTIA2_DB] SELECT_SEEK db={db_name} handle={db_id:#010x} offset={offset} mode={mode:#x} old_read={} old_write={} len={} lr={:#010x} pc={:#010x}",
                    handle.read_cursor,
                    handle.write_cursor,
                    handle.buffer_len,
                    regs[14],
                    regs[15]
                );
            } else {
                tracing::debug!(
                    "[INOTIA2_DB] SELECT_SEEK db={db_name} handle={db_id:#010x} offset={offset} mode={mode:#x} old_read={} old_write={} len={}",
                    handle.read_cursor,
                    handle.write_cursor,
                    handle.buffer_len
                );
            }
        }

        // Phase 8.8 — Inotia 2 exposed the actual KTF stream-seek
        // contract used by the native stdio wrapper.
        //
        // client.bin1149832::0x122fa8 determines a stream length exactly like:
        //
        //   saved = seek(handle, 0, CUR);
        //   begin = seek(handle, 0, SET);
        //   end   = seek(handle, 0, END);
        //   seek(handle, saved, SET);
        //   return end - begin;
        //
        // The old WIE implementation treated modes 0/1/2 identically and
        // always returned 0. Consequently game.dat appeared to have length
        // zero to Inotia 2. Its resource-table loader then failed before
        // initializing the resource globals later dereferenced by 0x144a2c.
        //
        // Keep this behavior title-scoped while we validate it on-device.
        // Other KTF titles retain the established compatibility behavior,
        // including the Inotia 1 save-length return fix below.
        if pid == "PD007974" {
            let base: i64 = match mode {
                0 => 0,                         // SEEK_SET
                1 => handle.read_cursor as i64, // SEEK_CUR
                2 => handle.buffer_len as i64,  // SEEK_END
                _ => {
                    tracing::warn!(
                        "[INOTIA2_SEEK_FIX] db={db_name} unsupported mode={mode:#x} offset={rec_id}"
                    );
                    return Ok(-22); // M_E_BADRECID / invalid seek mode
                }
            };

            let target = base + rec_id as i64;
            if target < 0 || target > u32::MAX as i64 {
                tracing::warn!(
                    "[INOTIA2_SEEK_FIX] db={db_name} invalid target base={base} offset={rec_id} mode={mode:#x}"
                );
                return Ok(-22);
            }

            let target = target as u32;
            let old_read = handle.read_cursor;
            let old_write = handle.write_cursor;
            handle.read_cursor = target;
            handle.write_cursor = target;

            write_generic(context, db_id as _, handle)?;

            tracing::debug!(
                "[PHASE8_8] Inotia2 KTF stream-seek semantics active"
            );
            tracing::debug!(
                "[INOTIA2_SEEK_FIX] db={db_name} mode={mode:#x} offset={rec_id} old_read={old_read} old_write={old_write} len={} -> position={target}",
                handle.buffer_len
            );

            return Ok(target as i32);
        }

        handle.read_cursor = offset;
        handle.write_cursor = offset;

        // Phase 7.21: Inotia 1's native DB wrapper uses the return value of
        // this KTF stream-control slot to recover the logical record length.
        //
        // Disassembly of client.bin138532 shows:
        //
        //     ret = slot4(handle, 0, 0);
        //     logical_len = 0x140 - ret;
        //
        // The previous WIE implementation always returned 0, so Continue
        // always derived 0x140 / 320 bytes even after Inotia had saved a
        // 324-byte post-Terry record.  That caused the final native validator
        // to reject the slot.  For the affected title/save record, return the
        // signed delta from the 320-byte baseline:
        //
        //     len 320 ->  0
        //     len 324 -> -4
        //     len 328 -> -8
        //
        // Empty/newly-created records and all other titles/databases retain
        // the existing return value of 0. Cursor behavior is unchanged.
        //
        // Use the exact stored length for every non-empty Inotia 1 save0
        // record.  Do not impose a size threshold in either direction:
        //
        //     len 300 -> +20 -> native length 300
        //     len 320 ->   0 -> native length 320
        //     len 544 -> -224 -> native length 544
        //
        // This makes the wrapper follow the record as it grows or shrinks
        // during gameplay instead of depending on guessed milestones.
        // The database quota is far below i32::MAX, so the signed delta is
        // safe for every record WIE can persist here.
        let inotia_record_len_delta =
            pid == "PD005362"
                && db_name == "save0.dat"
                && offset == 0
                && mode == 0
                && handle.buffer_len > 0;

        let result = if inotia_record_len_delta {
            320i32 - handle.buffer_len as i32
        } else {
            0
        };

        write_generic(context, db_id as _, handle)?;

        if inotia_record_len_delta {
            tracing::info!(
                "[INOTIA1_SEEK_FIX] db={db_name} len={} baseline=320 return={result} expected_read_len={}",
                handle.buffer_len,
                320i32 - result
            );
        }

        return Ok(result);
    }

    Ok(-22) // M_E_BADRECID
}

/// Slot 5 — KTF custom `db_stat_by_name`. From observed call shape:
///
/// ```text
/// int32 v2[3];
/// ret = slot5(name_ptr, &v2, mode, fn_self_ptr);
/// if (ret == 0 && v2[2] > 0xC7) "valid save";
/// ```
///
/// Takes a name plus a 12-byte (3-int) output struct, and returns 0 when
/// the DB exists with a non-trivial payload. The third int is treated as a
/// size threshold (must exceed 199 bytes). We fill the struct with
/// `{0, 0, record_size}` and return 0 on hit, -22 on miss.
pub async fn stat_by_name_ktf(context: &mut dyn WIPICContext, name_ptr: WIPICWord, out_buf: WIPICWord, mode: i32, _arg3: i32) -> Result<i32> {
    let name = match read_null_terminated_string_bytes(context, name_ptr) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => return Ok(-22),
        },
        Err(_) => return Ok(-22),
    };

    let system = context.system();
    let pid = system.pid().to_owned();
    let exists = system.platform().database_repository().exists(&name, &pid).await;
    if !exists {
        if pid == "PD007974" {
            tracing::debug!("[INOTIA2_DB] STAT db={name} mode={mode} -> -22 (not found)");
        }
        if pid == "PD005362" {
            tracing::info!("[INOTIA1_META] STAT db={name} mode={mode} -> -22 (not found)");
        } else {
            tracing::debug!("db.stat_by_name({name:?}, mode={mode}) -> -22 (not found)");
        }
        return Ok(-22);
    }

    // Pull record 1's size as the "valid save" indicator the game checks
    // against 0xC7 in v2[2].
    let db = system.platform().database_repository().open(&name, &pid).await;
    let record_size = db.get(1).await.map(|x| x.len() as u32).unwrap_or(0);

    if out_buf != 0 {
        write_generic(context, out_buf, 0u32)?;
        write_generic(context, out_buf + 4, 0u32)?;
        write_generic(context, out_buf + 8, record_size)?;
    }

    if pid == "PD007974" {
        tracing::debug!(
            "[INOTIA2_DB] STAT db={name} mode={mode} record_size={record_size} -> 0"
        );
    }
    if pid == "PD005362" {
        tracing::info!(
            "[INOTIA1_META] STAT db={name} mode={mode} record_size={record_size} -> 0"
        );
    } else {
        tracing::debug!("db.stat_by_name({name:?}, mode={mode}) -> 0 (size={record_size})");
    }
    Ok(0)
}

/// KTF custom slot 16 — `MC_dbExists(name)`. Observed call shape across
/// multiple titles is `(name_ptr, 1, size_hint_or_zero, callback_garbage)`.
/// Titles call it before deciding whether to take the load or fresh-init
/// path. Returning 1 unconditionally makes them try to load nonexistent
/// state on first run and trip later, so we read the C string at `a0` and
/// answer based on the real persisted state.
pub async fn exists_database_ktf(context: &mut dyn WIPICContext, name_ptr: WIPICWord, _arg1: i32, _arg2: i32) -> Result<i32> {
    let name = match read_null_terminated_string_bytes(context, name_ptr) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => {
                tracing::warn!("MC_dbExists invalid utf8 name @ {name_ptr:#x}, defaulting to 0");
                return Ok(0);
            }
        },
        Err(_) => {
            tracing::warn!("MC_dbExists unreadable name @ {name_ptr:#x}, defaulting to 0");
            return Ok(0);
        }
    };

    let system = context.system();
    let pid = system.pid().to_owned();
    let exists = system.platform().database_repository().exists(&name, &pid).await;

    let result = if exists { 1 } else { 0 };
    if pid == "PD007974" {
        tracing::debug!(
            "[INOTIA2_DB] EXISTS_KTF db={name} exists={exists} -> {result}"
        );
    }
    if pid == "PD005362" {
        tracing::info!(
            "[INOTIA1_META] EXISTS_KTF db={name} exists={exists} -> {result}"
        );
    } else {
        tracing::debug!("MC_dbExists({name:?}) -> {result}");
    }
    Ok(result)
}

/// Read a `DatabaseHandle` from guest memory if `db_id` looks like one.
///
/// Returns `Ok(None)` for any pointer that's obviously not a handle —
/// out-of-range, missing the magic sentinel — so callers can return
/// `M_E_INVALIDHANDLE` instead of panicking on garbage input.
fn load_handle(context: &mut dyn WIPICContext, db_id: i32) -> Result<Option<DatabaseHandle>> {
    if db_id < 0x10000 {
        return Ok(None);
    }
    let handle: DatabaseHandle = read_generic(context, db_id as _)?;
    if handle.magic != DATABASE_HANDLE_MAGIC {
        return Ok(None);
    }
    Ok(Some(handle))
}

fn handle_name(handle: &DatabaseHandle) -> &str {
    let name_length = handle.name.iter().position(|&c| c == 0).unwrap_or(handle.name.len());
    str::from_utf8(&handle.name[..name_length]).unwrap_or("<invalid>")
}

fn inotia_fingerprint(data: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

async fn open_db_for_handle(context: &mut dyn WIPICContext, handle: &DatabaseHandle) -> Option<Box<dyn Database>> {
    let name_length = handle.name.iter().position(|&c| c == 0).unwrap_or(handle.name.len());
    let db_name = str::from_utf8(&handle.name[..name_length]).ok()?;

    let system = context.system();
    let pid = system.pid().to_owned();

    Some(system.platform().database_repository().open(db_name, &pid).await)
}

async fn get_database_from_db_id(context: &mut dyn WIPICContext, db_id: i32) -> Result<Option<Box<dyn Database>>> {
    let Some(handle) = load_handle(context, db_id)? else {
        return Ok(None);
    };
    Ok(open_db_for_handle(context, &handle).await)
}

async fn read_packaged_database(context: &mut dyn WIPICContext, name: &str) -> Result<Option<Vec<u8>>> {
    if context.get_resource_size(name).await?.is_none() {
        return Ok(None);
    }

    Ok(Some(context.read_resource(name).await?))
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;

    use test_utils::TestPlatform;
    use wie_backend::{DefaultTaskRunner, System};
    use wie_util::{ByteRead, ByteWrite};

    use crate::context::test::TestContext;

    use super::{
        KTF_DATABASE_STORAGE_LIMIT, delete_database, exists_database, list_databases, list_record_info, open_database, select_record, stream_read,
        stream_write, update_record,
    };

    #[futures_test::test]
    async fn ktf_available_database_storage_tracks_app_usage() {
        let mut context = database_test_context();
        assert_eq!(list_databases(&mut context).await.unwrap(), KTF_DATABASE_STORAGE_LIMIT as i32);

        let db_id = open_test_database(&mut context).await;
        context.write_bytes(0x2000, &[1, 2, 3, 4]).unwrap();
        assert_eq!(stream_write(&mut context, db_id, 0x2000, 4).await.unwrap(), 4);

        assert_eq!(list_databases(&mut context).await.unwrap(), KTF_DATABASE_STORAGE_LIMIT as i32 - 4);
    }

    #[futures_test::test]
    async fn lgt_exists_database_reports_missing_and_existing_database() {
        let mut context = database_test_context();
        context.write_bytes(0x1000, b"records\0").unwrap();

        assert_eq!(exists_database(&mut context, 0x1000, 1).await.unwrap(), -12);
        let db_id = open_database(&mut context, 0x1000, 0, 0).await.unwrap();
        context.write_bytes(0x2000, &[1]).unwrap();
        assert_eq!(stream_write(&mut context, db_id, 0x2000, 1).await.unwrap(), 1);
        assert_eq!(exists_database(&mut context, 0x1000, 1).await.unwrap(), 0);
    }

    #[futures_test::test]
    async fn lgt_create_mode_materializes_empty_database() {
        let mut context = database_test_context();
        context.write_bytes(0x1000, b"records\0").unwrap();

        assert_eq!(exists_database(&mut context, 0x1000, 1).await.unwrap(), -12);
        let db_id = open_database(&mut context, 0x1000, 4, 0).await.unwrap();
        assert!(db_id > 0);
        assert_eq!(exists_database(&mut context, 0x1000, 1).await.unwrap(), 0);
    }

    #[futures_test::test]
    async fn lgt_update_and_select_record_use_standard_record_ids() {
        let mut context = database_test_context();
        let db_id = open_test_database(&mut context).await;
        context.write_bytes(0x2000, &[1, 2, 3]).unwrap();
        assert_eq!(stream_write(&mut context, db_id, 0x2000, 3).await.unwrap(), 3);
        context.write_bytes(0x2010, &[4, 5]).unwrap();

        assert_eq!(update_record(&mut context, db_id, 1, 0x2010, 2).await.unwrap(), 0);
        assert_eq!(select_record(&mut context, db_id, 1, 0x2100, 2).await.unwrap(), 0);

        let mut data = [0; 2];
        context.read_bytes(0x2100, &mut data).unwrap();
        assert_eq!(data, [4, 5]);
    }

    #[futures_test::test]
    async fn lgt_list_record_info_and_delete_database_use_database_name() {
        let mut context = database_test_context();
        let db_id = open_test_database(&mut context).await;
        context.write_bytes(0x2000, &[1, 2, 3, 4]).unwrap();
        assert_eq!(stream_write(&mut context, db_id, 0x2000, 4).await.unwrap(), 4);

        assert_eq!(list_record_info(&mut context, 0x1000, 0x2100, 1).await.unwrap(), 0);
        let mut entry = [0; 12];
        context.read_bytes(0x2100, &mut entry).unwrap();
        assert_eq!(u32::from_le_bytes(entry[0..4].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(entry[8..12].try_into().unwrap()), 4);

        assert_eq!(delete_database(&mut context, 0x1000, 1).await.unwrap(), 0);
        assert_eq!(exists_database(&mut context, 0x1000, 1).await.unwrap(), -12);
    }

    #[futures_test::test]
    async fn lgt_open_database_materializes_packaged_database() {
        let mut context = database_test_context().with_resource("kickass", b"seed-data");
        context.write_bytes(0x1000, b"kickass\0").unwrap();

        assert_eq!(exists_database(&mut context, 0x1000, 1).await.unwrap(), 0);
        let db_id = open_database(&mut context, 0x1000, 1, 0).await.unwrap();
        assert!(db_id > 0);
        assert_eq!(stream_read(&mut context, db_id, 0x2000, 9).await.unwrap(), 9);

        let mut data = [0; 9];
        context.read_bytes(0x2000, &mut data).unwrap();
        assert_eq!(&data, b"seed-data");
    }

    fn database_test_context() -> TestContext {
        let system = System::new(Box::new(TestPlatform::new()), "test-pid", "test-aid", DefaultTaskRunner);
        TestContext::with_system(system)
    }

    async fn open_test_database(context: &mut TestContext) -> i32 {
        context.write_bytes(0x1000, b"records\0").unwrap();
        open_database(context, 0x1000, 0, 0).await.unwrap()
    }
}
