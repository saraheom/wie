use wipi_types::wipic::WIPICWord;

use wie_util::{Result, read_null_terminated_string_bytes};

use crate::context::WIPICContext;

pub async fn htons(_context: &mut dyn WIPICContext, val: WIPICWord) -> Result<WIPICWord> {
    tracing::debug!("MC_utilHtons({val})");

    Ok((val as u16).to_be() as _) // XXX we're always on little endian
}

// Phase 8.14 — implement the KTF utility call used immediately after
// MC_netSocket by Inotia 1's legacy cash-shop client.  Static analysis of
// PD005362 maps the intervening interface call to utility slot 4
// (MC_utilInetAddrInt).  Phase 8.13 correctly added the later network slot 30,
// but the guest never reached it because this utility slot was still a fatal
// stub.
//
// For normal dotted-IPv4 input, provide the standard string-to-address
// conversion.  The obsolete Inotia 1 service is deliberately kept offline; if
// its historical endpoint is not a dotted address, return a deterministic
// loopback placeholder rather than attempting DNS or contacting a live host.
// This only relaxes the fallback for the exact Inotia 1 AID/PID.
pub async fn inet_addr_int(context: &mut dyn WIPICContext, ptr_addr: WIPICWord) -> Result<WIPICWord> {
    const INOTIA1_AID: &str = "010100D3";
    const INOTIA1_PID: &str = "PD005362";
    const LOOPBACK: WIPICWord = 0x7f00_0001;

    let inotia1 = {
        let system = context.system();
        system.aid() == INOTIA1_AID && system.pid() == INOTIA1_PID
    };

    let bytes = match read_null_terminated_string_bytes(context, ptr_addr) {
        Ok(bytes) => bytes,
        Err(err) if inotia1 => {
            tracing::warn!(
                "[PHASE8_14_INOTIA1_INETADDR] unreadable legacy endpoint ptr={ptr_addr:#010x}: {err:?}; use loopback placeholder {LOOPBACK:#010x}"
            );
            return Ok(LOOPBACK);
        }
        Err(err) => return Err(err),
    };

    let mut octets = [0u8; 4];
    let mut count = 0usize;
    let mut valid = true;
    for part in bytes.split(|byte| *byte == b'.') {
        if count >= 4 || part.is_empty() || part.len() > 3 {
            valid = false;
            break;
        }

        let mut value: u16 = 0;
        for byte in part {
            if !byte.is_ascii_digit() {
                valid = false;
                break;
            }
            value = value * 10 + (*byte - b'0') as u16;
            if value > 255 {
                valid = false;
                break;
            }
        }
        if !valid {
            break;
        }

        octets[count] = value as u8;
        count += 1;
    }

    if valid && count == 4 {
        let value = u32::from_be_bytes(octets);
        tracing::info!(
            "[PHASE8_14_INETADDR] ptr={ptr_addr:#010x} bytes={bytes:?} -> {value:#010x}"
        );
        return Ok(value);
    }

    if inotia1 {
        tracing::info!(
            "[PHASE8_14_INOTIA1_INETADDR] legacy endpoint bytes={bytes:?} is not dotted IPv4 -> offline loopback {LOOPBACK:#010x}"
        );
        return Ok(LOOPBACK);
    }

    // inet_addr-style failure value.
    Ok(u32::MAX)
}
