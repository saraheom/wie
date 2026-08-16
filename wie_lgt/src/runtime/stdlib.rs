use alloc::{format, string::String, vec};
use chrono::{DateTime, Datelike, FixedOffset, TimeZone, Timelike};
use core::cmp::min;

use wie_backend::System;
use wie_core_arm::{Allocator, ArmCore, EmulatedFunction, ResultWriter, SvcId, stdlib};
use wie_util::{ByteRead, ByteWrite, Result, WieError, read_generic, read_null_terminated_string_bytes, write_generic, write_null_terminated_string_bytes};

use crate::runtime::{SVC_CATEGORY_STDLIB, svc_ids::StdlibSvcId};


fn trace_unknown_stdlib(core: &ArmCore, id: u32) {
    let ctx = core.save_context();
    tracing::error!(
        "[LGT_ABI] unknown stdlib id={:#x} pc={:#x} lr={:#x} sp={:#x} cpsr={:#x} r0={:#x} r1={:#x} r2={:#x} r3={:#x} r4={:#x} r5={:#x} r6={:#x} r7={:#x} r8={:#x} r9={:#x} r10={:#x} r11={:#x} r12={:#x}",
        id, ctx.pc, ctx.lr, ctx.sp, ctx.cpsr, ctx.r0, ctx.r1, ctx.r2, ctx.r3, ctx.r4, ctx.r5, ctx.r6, ctx.r7, ctx.r8, ctx.sb, ctx.sl, ctx.fp, ctx.ip
    );

    let mut stack_words = [0u32; 12];
    for (i, word) in stack_words.iter_mut().enumerate() {
        *word = read_generic(core, ctx.sp.wrapping_add((i as u32) * 4)).unwrap_or(0xdead_beef);
    }
    tracing::error!("[LGT_ABI] stdlib id={id:#x} stack_words={stack_words:#x?}");

    let caller = ctx.lr & !1;
    let base = caller.saturating_sub(24);
    let mut caller_words = [0u32; 16];
    for (i, word) in caller_words.iter_mut().enumerate() {
        *word = read_generic(core, base.wrapping_add((i as u32) * 4)).unwrap_or(0xdead_beef);
    }
    tracing::error!("[LGT_ABI] stdlib id={id:#x} caller={caller:#x} caller_words_base={base:#x} words={caller_words:#x?}");

    for (name, ptr) in [("r0", ctx.r0), ("r1", ctx.r1), ("r2", ctx.r2), ("r3", ctx.r3)] {
        if ptr >= 0x10000 {
            let mut words = [0u32; 8];
            let mut ok = true;
            for (i, word) in words.iter_mut().enumerate() {
                match read_generic(core, ptr.wrapping_add((i as u32) * 4)) {
                    Ok(v) => *word = v,
                    Err(_) => { ok = false; break; }
                }
            }
            if ok {
                tracing::error!("[LGT_ABI] stdlib id={id:#x} {name}_preview ptr={ptr:#x} words={words:#x?}");
            }
        }
    }
}

pub fn register_stdlib_svc_handler(core: &mut ArmCore, system: &System) -> Result<()> {
    async fn handle_stdlib_svc(core: &mut ArmCore, system: &mut System, id: SvcId) -> Result<()> {
        let (_, lr) = core.read_pc_lr()?;

        match id.0 {
            x if x == StdlibSvcId::Unk2 as u32 => EmulatedFunction::call(&unk2, core, &mut ()).await?.write(core, lr),
            x if x == StdlibSvcId::Sprintf as u32 => EmulatedFunction::call(&sprintf_lgt, core, &mut ()).await?.write(core, lr),
            x if x == StdlibSvcId::Atoi as u32 => EmulatedFunction::call(&atoi, core, &mut ()).await?.write(core, lr),
            x if x == StdlibSvcId::Strcpy as u32 => EmulatedFunction::call(&stdlib::strcpy, core, &mut ()).await?.write(core, lr),
            x if x == StdlibSvcId::Strncpy as u32 => EmulatedFunction::call(&strncpy, core, &mut ()).await?.write(core, lr),
            x if x == StdlibSvcId::Strcat as u32 => EmulatedFunction::call(&strcat, core, &mut ()).await?.write(core, lr),
            x if x == StdlibSvcId::Strcmp as u32 => EmulatedFunction::call(&strcmp, core, &mut ()).await?.write(core, lr),
            x if x == StdlibSvcId::Unk4 as u32 => EmulatedFunction::call(&unk4, core, &mut ()).await?.write(core, lr),
            x if x == StdlibSvcId::Unk5 as u32 => EmulatedFunction::call(&unk5, core, &mut ()).await?.write(core, lr),
            x if x == StdlibSvcId::Strlen as u32 => EmulatedFunction::call(&stdlib::strlen, core, &mut ()).await?.write(core, lr),
            x if x == StdlibSvcId::Memcpy as u32 => EmulatedFunction::call(&stdlib::memcpy, core, &mut ()).await?.write(core, lr),
            x if x == StdlibSvcId::Memcmp as u32 => EmulatedFunction::call(&memcmp_lgt, core, &mut ()).await?.write(core, lr),
            x if x == StdlibSvcId::Memset as u32 => EmulatedFunction::call(&stdlib::memset, core, &mut ()).await?.write(core, lr),
            x if x == StdlibSvcId::Time as u32 => EmulatedFunction::call(&time, core, system).await?.write(core, lr),
            x if x == StdlibSvcId::Localtime as u32 => EmulatedFunction::call(&localtime, core, &mut ()).await?.write(core, lr),
            x if x == StdlibSvcId::Unk3 as u32 => EmulatedFunction::call(&unk3, core, &mut ()).await?.write(core, lr),
            _ => {
                trace_unknown_stdlib(core, id.0);
                Err(WieError::FatalError(format!("Unknown lgt stdlib import: {:#x}", id.0)))
            },
        }
    }

    core.register_svc_handler(SVC_CATEGORY_STDLIB, handle_stdlib_svc, system)
}


async fn memcmp_lgt(core: &mut ArmCore, _: &mut (), ptr_a: u32, ptr_b: u32, size: u32) -> Result<u32> {
    tracing::debug!("[LGT_COMPAT] stdlib 0x416 memcmp({ptr_a:#x}, {ptr_b:#x}, {size:#x})");

    if size == 0 {
        return Ok(0);
    }

    // Keep guest-controlled allocations bounded. Real game calls are tiny; this
    // protects the host from a corrupt length while retaining normal C semantics.
    const MAX_COMPARE: u32 = 32 * 1024 * 1024;
    if size > MAX_COMPARE {
        return Err(WieError::FatalError(format!("memcmp length too large: {size:#x}")));
    }

    let mut a = vec![0u8; size as usize];
    let mut b = vec![0u8; size as usize];
    core.read_bytes(ptr_a, &mut a)?;
    core.read_bytes(ptr_b, &mut b)?;

    for (left, right) in a.iter().zip(b.iter()) {
        if left != right {
            return Ok(((*left as i32) - (*right as i32)) as u32);
        }
    }

    Ok(0)
}

#[allow(clippy::too_many_arguments)]
async fn sprintf_lgt(
    core: &mut ArmCore,
    _: &mut (),
    dest: u32,
    ptr_format: u32,
    a0: u32,
    a1: u32,
    a2: u32,
    a3: u32,
    a4: u32,
    a5: u32,
) -> Result<u32> {
    tracing::info!(
        "[LGT_COMPAT] stdlib 0x3f7 sprintf({dest:#x}, {ptr_format:#x}, {a0:#x}, {a1:#x}, {a2:#x}, {a3:#x}, {a4:#x}, {a5:#x})"
    );

    let format_bytes = read_null_terminated_string_bytes(core, ptr_format)?;
    let format_string = encoding_rs::EUC_KR.decode(&format_bytes).0.into_owned();
    let args = [a0, a1, a2, a3, a4, a5];
    let result = format_guest_printf(core, &format_string, &args)?;
    let encoded = encoding_rs::EUC_KR.encode(&result).0;
    let byte_len = encoded.len() as u32;
    write_null_terminated_string_bytes(core, dest, &encoded)?;

    tracing::info!("[LGT_COMPAT] stdlib 0x3f7 sprintf -> {byte_len} bytes: {result:?}");
    Ok(byte_len)
}

fn format_guest_printf(core: &ArmCore, format_string: &str, args: &[u32]) -> Result<String> {
    const MAX_WIDTH: usize = 4096;
    let mut result = String::with_capacity(format_string.len());
    let mut chars = format_string.chars();
    let mut arg_index = 0usize;

    let next_arg = |index: &mut usize| -> u32 {
        let value = args.get(*index).copied().unwrap_or(0);
        *index = (*index).saturating_add(1);
        value
    };

    while let Some(ch) = chars.next() {
        if ch != '%' {
            result.push(ch);
            continue;
        }

        let mut flag = None;
        let mut width: Option<usize> = None;
        let mut longs = 0u32;
        loop {
            let Some(spec) = chars.next() else {
                result.push('%');
                break;
            };

            match spec {
                '%' => { result.push('%'); break; }
                'l' => { longs += 1; }
                '0' if width.is_none() => { flag = Some('0'); }
                '0'..='9' => {
                    width = Some(width.unwrap_or(0).saturating_mul(10).saturating_add(spec.to_digit(10).unwrap() as usize));
                }
                'd' | 'u' | 'x' | 'X' => {
                    let raw = if longs >= 2 {
                        let low = next_arg(&mut arg_index) as u64;
                        let high = next_arg(&mut arg_index) as u64;
                        (high << 32) | low
                    } else {
                        next_arg(&mut arg_index) as u64
                    };
                    let width = width.unwrap_or(0).min(MAX_WIDTH);
                    match spec {
                        'd' => {
                            let value = if longs >= 2 { raw as i64 } else { raw as u32 as i32 as i64 };
                            if flag == Some('0') { result += &format!("{value:0width$}"); } else { result += &format!("{value:width$}"); }
                        }
                        'u' => {
                            if flag == Some('0') { result += &format!("{raw:0width$}"); } else { result += &format!("{raw:width$}"); }
                        }
                        'x' => {
                            if flag == Some('0') { result += &format!("{raw:0width$x}"); } else { result += &format!("{raw:width$x}"); }
                        }
                        'X' => {
                            if flag == Some('0') { result += &format!("{raw:0width$X}"); } else { result += &format!("{raw:width$X}"); }
                        }
                        _ => unreachable!(),
                    }
                    break;
                }
                's' => {
                    let ptr = next_arg(&mut arg_index);
                    if ptr == 0 {
                        result.push_str("(null)");
                    } else {
                        let bytes = read_null_terminated_string_bytes(core, ptr)?;
                        result.push_str(&encoding_rs::EUC_KR.decode(&bytes).0);
                    }
                    break;
                }
                'c' => {
                    result.push(next_arg(&mut arg_index) as u8 as char);
                    break;
                }
                other => {
                    tracing::warn!("unsupported LGT sprintf format specifier: %{other}");
                    result.push('%');
                    result.push(other);
                    break;
                }
            }
        }
    }

    Ok(result)
}

async fn strncpy(core: &mut ArmCore, _: &mut (), ptr_dst: u32, ptr_src: u32, size: u32) -> Result<()> {
    tracing::debug!("strncpy({ptr_dst:#x}, {ptr_src:#x}, {size:#x})");

    let src = read_null_terminated_string_bytes(core, ptr_src)?;

    let size_to_copy = min(size, src.len() as u32);
    let bytes = &src[..size_to_copy as usize];

    core.write_bytes(ptr_dst, bytes)?;

    Ok(())
}

async fn strcat(core: &mut ArmCore, _: &mut (), ptr_dst: u32, ptr_src: u32) -> Result<()> {
    tracing::debug!("strcat({ptr_dst:#x}, {ptr_src:#x})");

    let src = read_null_terminated_string_bytes(core, ptr_src)?;
    let dst = read_null_terminated_string_bytes(core, ptr_dst)?;

    let offset = dst.len();
    write_null_terminated_string_bytes(core, ptr_dst + offset as u32, &src)?;

    Ok(())
}

async fn strcmp(core: &mut ArmCore, _: &mut (), ptr_str1: u32, ptr_str2: u32) -> Result<u32> {
    tracing::debug!("strcmp({ptr_str1:#x}, {ptr_str2:#x})");

    let str1 = read_null_terminated_string_bytes(core, ptr_str1)?;
    let str2 = read_null_terminated_string_bytes(core, ptr_str2)?;

    Ok(str1.cmp(&str2) as u32)
}

async fn atoi(core: &mut ArmCore, _: &mut (), ptr_str: u32) -> Result<u32> {
    tracing::debug!("atoi({ptr_str:#x})");

    let string = read_null_terminated_string_bytes(core, ptr_str)?;
    let string = String::from_utf8(string).unwrap();

    Ok(string.parse().unwrap_or(0))
}

async fn time(core: &mut ArmCore, system: &mut System, ptr_time: u32) -> Result<u32> {
    let epoch_seconds = (system.platform().now().raw() / 1000) as u32;
    tracing::debug!("time({ptr_time:#x}) -> {epoch_seconds}");

    if ptr_time != 0 {
        write_generic(core, ptr_time, epoch_seconds)?;
    }

    Ok(epoch_seconds)
}

// TODO is this method better suit on wie_backend?
async fn localtime(core: &mut ArmCore, _: &mut (), ptr_time: u32) -> Result<u32> {
    tracing::debug!("localtime({ptr_time:#x})");

    // TODO we need static buffer
    let result = Allocator::alloc(core, 0x2c)?;
    let time: u32 = read_generic(core, ptr_time)?;

    // TODO kst only for now
    let kst = FixedOffset::east_opt(9 * 3600).unwrap();
    let dt: DateTime<FixedOffset> = kst.timestamp_opt(time as _, 0).unwrap();

    // TODO tm struct
    write_generic(core, result, dt.second() as u32)?;
    write_generic(core, result + 0x04, dt.minute() as u32)?;
    write_generic(core, result + 0x08, dt.hour() as u32)?;
    write_generic(core, result + 0x0c, dt.day() as u32)?;
    write_generic(core, result + 0x10, (dt.month() as u32) - 1)?; // months since January
    write_generic(core, result + 0x14, (dt.year() as u32) - 1900)?; // years since 1900
    write_generic(core, result + 0x18, dt.weekday().num_days_from_sunday() as u32)?; // days since Sunday
    write_generic(core, result + 0x1c, dt.ordinal() as u32)?; // days since January 1
    write_generic(core, result + 0x20, 0u32)?; // DST flag
    write_generic(core, result + 0x24, kst.local_minus_utc() as u32)?; // timezone offset in seconds
    write_generic(core, result + 0x28, 0u32)?; // timezone abbreviation ptr

    Ok(result)
}

async fn unk2(_core: &mut ArmCore, _: &mut (), a0: u32) -> Result<()> {
    tracing::warn!("unk2({a0:#x})");

    // error exit?

    Ok(())
}

async fn unk3(core: &mut ArmCore, _: &mut (), a0: u32) -> Result<()> {
    tracing::warn!("unk3({a0:#x})");

    let _: () = core.run_function(a0, &[]).await?;

    Ok(())
}

async fn unk4(_core: &mut ArmCore, _: &mut (), a0: u32, a1: u32, a2: u32, a3: u32) -> Result<()> {
    tracing::warn!("unk4({a0:#x}, {a1:#x}, {a2:#x}, {a3:#x})");

    Ok(())
}

async fn unk5(_core: &mut ArmCore, _: &mut (), a0: u32, a1: u32) -> Result<()> {
    tracing::warn!("unk5({a0:#x}, {a1:#x})");
    // strstr??

    Ok(())
}
