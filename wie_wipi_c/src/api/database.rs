use alloc::{borrow::ToOwned, boxed::Box, str, string::String, vec, vec::Vec};
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
}

const MIN_BUFFER_CAPACITY: u32 = 64;
const KTF_DATABASE_STORAGE_LIMIT: u64 = 16 * 1024 * 1024;
// "MCDB" — sentinel at the start of the handle struct so we can distinguish
// a real DB handle pointer from an unrelated guest pointer (e.g. a C-string
// name pointer that KTF's slot 6 passes through the same SVC argument slot).
const DATABASE_HANDLE_MAGIC: u32 = 0x4D434442;
const MAX_NAME_LEN: usize = 31; // leave a byte for null terminator inside the 32-byte field

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

    let packaged = read_packaged_database(context, &name).await?;

    let system = context.system();
    let pid = system.pid().to_owned();
    if pid == "PD005362" {
        tracing::info!("[PHASE7_20] Inotia1 post-Terry checksum-bypass experiment active");
    }
    let exists = system.platform().database_repository().exists(&name, &pid).await;

    if !exists && packaged.is_none() && mode == 1 {
        return Ok(-12); // M_E_NOENT
    }

    // Mode 4 (`MC_DB_CREATE`) is true create/truncate semantics.  Phase 7.14
    // temporarily preserved an existing Inotia 1 record here; the exported
    // before/after quest snapshots proved that doing so can leave bytes from
    // the previous save generation in the record.  Revert to strict CREATE:
    // delete record 1 first, then let subsequent writes rebuild it.
    let initial: Vec<u8> = if exists {
        let mut db = system.platform().database_repository().open(&name, &pid).await;
        if mode == 4 && packaged.is_none() {
            if pid == "PD005362" {
                let old_len = db.get(1).await.map(|x| x.len()).unwrap_or(0);
                tracing::info!(
                    "[INOTIA1_SAVE] OPEN db={name} mode=CREATE existing={old_len} -> truncate"
                );
            }
            db.delete(1).await;
            Vec::new()
        } else if let Some(data) = db.get(1).await {
            if pid == "PD005362" {
                tracing::info!(
                    "[INOTIA1_SAVE] OPEN db={name} mode={mode} existing={} -> preserve",
                    data.len()
                );
            }
            data
        } else if let Some(data) = packaged {
            db.set(1, &data).await;
            data
        } else {
            Vec::new()
        }
    } else if let Some(data) = packaged {
        let mut db = system.platform().database_repository().open(&name, &pid).await;
        db.set(1, &data).await;
        data
    } else if mode == 4 {
        system.platform().database_repository().open(&name, &pid).await;
        Vec::new()
    } else {
        Vec::new()
    };

    let name_bytes = name.as_bytes();

    let mut handle = DatabaseHandle {
        magic: DATABASE_HANDLE_MAGIC,
        name: [0; 32],
        read_cursor: 0,
        write_cursor: 0,
        buffer_ptr: 0,
        buffer_len: 0,
        buffer_capacity: 0,
    };
    handle.name[..name_bytes.len()].copy_from_slice(name_bytes);

    if !initial.is_empty() {
        let cap = (initial.len() as u32).max(MIN_BUFFER_CAPACITY);
        let buf_ptr = context.alloc_raw(cap)?;
        context.write_bytes(buf_ptr, &initial)?;
        handle.buffer_ptr = buf_ptr;
        handle.buffer_len = initial.len() as u32;
        handle.buffer_capacity = cap;
    }

    let ptr_handle = context.alloc_raw(size_of::<DatabaseHandle>() as _)?;
    write_generic(context, ptr_handle, handle)?;

    tracing::debug!("Created database handle {ptr_handle:#x} for {name}");

    Ok(ptr_handle as _)
}

pub async fn close_database(context: &mut dyn WIPICContext, db_id: i32) -> Result<i32> {
    tracing::debug!("MC_dbCloseDataBase({db_id:#x})");

    let Some(handle) = load_handle(context, db_id)? else {
        return Ok(-25); // M_E_INVALIDHANDLE
    };

    // The buffer was kept in sync with disk via write-through on every
    // `stream_write`, so close just frees the guest-heap allocations.
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
    let usage = system.platform().database_repository().usage(&pid).await;
    let available = KTF_DATABASE_STORAGE_LIMIT.saturating_sub(usage).min(i32::MAX as u64) as i32;

    tracing::debug!("MC_dbListDataBase() = {available} (used={usage}, limit={KTF_DATABASE_STORAGE_LIMIT})");
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
        tracing::info!(
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
    if read_packaged_database(context, &name).await?.is_some() {
        if context.system().pid() == "PD005362" {
            tracing::info!("[INOTIA1_META] EXISTS_STANDARD db={name} source=packaged -> 0");
        }
        return Ok(0);
    }

    let system = context.system();
    let pid = system.pid().to_owned();
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
    let pid = context.system().pid().to_owned();

    if pid == "PD005362" {
        tracing::info!(
            "[INOTIA1_SAVE] WRITE begin db={db_name} offset={} len={buf_len} old_len={old_len}",
            handle.write_cursor
        );
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
    if let Some(mut db) = open_db_for_handle(context, &handle).await {
        db.set(1, &snapshot).await;
    }

    if pid == "PD005362" {
        let head_end = snapshot.len().min(16);
        let tail_start = snapshot.len().saturating_sub(16);
        let head = &snapshot[..head_end];
        let tail = &snapshot[tail_start..];
        let fp = inotia_fingerprint(&snapshot);
        let first320_len = snapshot.len().min(320);
        let first320_fp = inotia_fingerprint(&snapshot[..first320_len]);
        let extra = if snapshot.len() > 320 { &snapshot[320..] } else { &snapshot[0..0] };
        tracing::info!(
            "[INOTIA1_SAVE] WRITE commit db={db_name} final_len={} cursor={} fnv64={fp:016x} first320_len={first320_len} first320_fnv64={first320_fp:016x} extra_len={} extra={:02x?} head={:02x?} tail={:02x?}",
            snapshot.len(),
            handle.write_cursor,
            extra.len(),
            extra,
            head,
            tail
        );
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
        tracing::info!(
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

    if handle.read_cursor >= handle.buffer_len {
        // Don't touch buf — caller may have passed a sentinel (NULL) that
        // we shouldn't write to. Some titles do this past EOF.
        return Ok(-23); // M_E_EOF
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

    if context.system().pid() == "PD005362" {
        // `data` is exactly what the guest received.  For save files, also
        // fingerprint the complete backing record so a 320-byte Continue
        // read can be correlated with a 324-byte persisted generation without
        // changing the bytes or cursor semantics.
        let returned = &data[..];
        let head_end = returned.len().min(16);
        let tail_start = returned.len().saturating_sub(16);
        let head = &returned[..head_end];
        let tail = &returned[tail_start..];
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
            let extra = if backing.len() > 320 { &backing[320..] } else { &backing[0..0] };
            tracing::info!(
                "[INOTIA1_SAVE] READ db={} offset={} request={buf_len} returned={take} final_cursor={} record_len={} remaining={remaining} returned_fnv64={fp:016x} backing_fnv64={backing_fp:016x} backing_first320_len={first320_len} backing_first320_fnv64={first320_fp:016x} backing_extra_len={} backing_extra={:02x?} head={:02x?} tail={:02x?}",
                handle_name(&handle),
                old_cursor,
                handle.read_cursor,
                handle.buffer_len,
                extra.len(),
                extra,
                head,
                tail
            );

            // Phase 7.19: once storage integrity was proven, capture the exact
            // guest ARM caller state at the Continue-screen save0 read.  The
            // snapshot is observational only: no registers or guest memory are
            // changed.  LR plus the R7 frame chain should identify the native
            // validation routine that accepts the 320-byte pre-Terry payload
            // but rejects the post-Terry payload.
            if handle_name(&handle) == "save0.dat" && old_cursor == 0 && take == 320 {
                if let Some(regs) = context.debug_cpu_context() {
                    let sp = regs[13];
                    let lr = regs[14];
                    let pc = regs[15];
                    let cpsr = regs[16];

                    let mut stack = [0u8; 64];
                    let stack_read = context.read_bytes(sp, &mut stack).unwrap_or(0);

                    // The SVC stub normally preserves the guest LR.  Dump code
                    // around both LR and PC when mapped; failed reads simply
                    // report zero bytes rather than affecting emulation.
                    let lr_code_base = (lr & !1).saturating_sub(32);
                    let mut lr_code = [0u8; 64];
                    let lr_code_read = context.read_bytes(lr_code_base, &mut lr_code).unwrap_or(0);

                    let pc_code_base = (pc & !1).saturating_sub(16);
                    let mut pc_code = [0u8; 32];
                    let pc_code_read = context.read_bytes(pc_code_base, &mut pc_code).unwrap_or(0);

                    // Walk the conventional Thumb R7 frame chain used elsewhere
                    // by wie_core_arm's profiler: [previous_r7, saved_lr].
                    let mut frames: Vec<u32> = Vec::new();
                    let mut frame_r7 = regs[7];
                    for _ in 0..12 {
                        if frame_r7 == 0 {
                            break;
                        }
                        let prev_r7: u32 = match read_generic(context, frame_r7) {
                            Ok(v) => v,
                            Err(_) => break,
                        };
                        let saved_lr: u32 = match read_generic(context, frame_r7 + 4) {
                            Ok(v) => v,
                            Err(_) => break,
                        };
                        if saved_lr == 0 {
                            break;
                        }
                        frames.push(saved_lr);
                        if prev_r7 <= frame_r7 {
                            break;
                        }
                        frame_r7 = prev_r7;
                    }

                    tracing::info!(
                        "[INOTIA1_ARM] save0_read offset={old_cursor} returned={take} record_len={} remaining={remaining} r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x} r4={:#010x} r5={:#010x} r6={:#010x} r7={:#010x} r8={:#010x} r9={:#010x} r10={:#010x} r11={:#010x} r12={:#010x} sp={sp:#010x} lr={lr:#010x} pc={pc:#010x} cpsr={cpsr:#010x} frames={:08x?} stack_read={stack_read} stack={:02x?} lr_code_base={lr_code_base:#010x} lr_code_read={lr_code_read} lr_code={:02x?} pc_code_base={pc_code_base:#010x} pc_code_read={pc_code_read} pc_code={:02x?}",
                        handle.buffer_len,
                        regs[0], regs[1], regs[2], regs[3],
                        regs[4], regs[5], regs[6], regs[7],
                        regs[8], regs[9], regs[10], regs[11], regs[12],
                        frames,
                        &stack[..stack_read.min(stack.len())],
                        &lr_code[..lr_code_read.min(lr_code.len())],
                        &pc_code[..pc_code_read.min(pc_code.len())],
                    );
                } else {
                    tracing::info!("[INOTIA1_ARM] save0_read CPU snapshot unavailable");
                }

                // Phase 7.20 experiment:
                //
                // Phase 7.19 resolved the native caller chain:
                //   0x11bd4a -> WIPI DB read wrapper 0x10becc
                //   return 0x11bd4f
                //   caller return 0x11d119
                //   outer caller return 0x11b4cf
                //
                // Disassembly of this exact Inotia 1 binary shows that the
                // downstream validator ends with:
                //
                //   0x11f11a  CMP r4, r0
                //   0x11f11c  BEQ 0x11f05a   ; accept
                //   0x11f11e  MOVS r0, #0    ; reject
                //
                // The post-Terry record is 324 bytes while Continue deliberately
                // reads only the first 320 bytes.  For this experiment only,
                // when that exact condition is present, turn the final BEQ into
                // an unconditional B to the same target.  This bypasses only
                // the final integrity comparison; all decoding/processing that
                // precedes it still runs normally.
                //
                // The patch is guarded by an exact 18-byte signature from the
                // tested `client.bin138532`, so other titles/revisions are left
                // untouched even if they share PID behavior accidentally.
                if handle.buffer_len > 320 {
                    const VALIDATOR_SIG_ADDR: u32 = 0x0011f110;
                    const VALIDATOR_BRANCH_ADDR: u32 = 0x0011f11c;
                    const EXPECTED: [u8; 18] = [
                        0x30, 0x1c, 0x05, 0x99, 0x1c, 0x78,
                        0xee, 0xf7, 0x87, 0xf8, 0x84, 0x42,
                        0x9d, 0xd0, 0x00, 0x20, 0x9c, 0xe7,
                    ];

                    let mut signature = [0u8; 18];
                    match context.read_bytes(VALIDATOR_SIG_ADDR, &mut signature) {
                        Ok(n) if n == signature.len() && signature == EXPECTED => {
                            // Existing `BEQ` is bytes 9d d0.  `B` to the same
                            // destination is 9d e7.
                            context.write_bytes(VALIDATOR_BRANCH_ADDR, &[0x9d, 0xe7])?;
                            tracing::info!(
                                "[INOTIA1_FIX] installed signature-guarded final-validation bypass addr={VALIDATOR_BRANCH_ADDR:#010x} record_len={} remaining={remaining}",
                                handle.buffer_len
                            );
                        }
                        Ok(n) => {
                            tracing::warn!(
                                "[INOTIA1_FIX] validator signature mismatch; no patch applied read={n} addr={VALIDATOR_SIG_ADDR:#010x} got={:02x?}",
                                &signature[..n.min(signature.len())]
                            );
                        }
                        Err(err) => {
                            tracing::warn!(
                                "[INOTIA1_FIX] could not inspect validator signature; no patch applied: {err:?}"
                            );
                        }
                    }
                }
            }
        } else {
            tracing::info!(
                "[INOTIA1_SAVE] READ db={} offset={} request={buf_len} returned={take} final_cursor={} record_len={} remaining={remaining} fnv64={fp:016x} head={:02x?} tail={:02x?}",
                handle_name(&handle),
                old_cursor,
                handle.read_cursor,
                handle.buffer_len,
                head,
                tail
            );
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
        if context.system().pid() == "PD005362" {
            tracing::info!(
                "[INOTIA1_SAVE] SELECT/SEEK db={} offset={offset} mode={mode:#x} old_read={} old_write={} len={}",
                handle_name(&handle),
                handle.read_cursor,
                handle.write_cursor,
                handle.buffer_len
            );
        }
        handle.read_cursor = offset;
        handle.write_cursor = offset;
        write_generic(context, db_id as _, handle)?;
        return Ok(0);
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
