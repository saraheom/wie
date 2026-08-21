# Phase 8.6 — Inotia 2 post-i_pack validation-global probe

This phase preserves:
- Phase 8.4 KTF packaged-database filesystem fallback;
- Phase 8.5 i_pack post-header probe;
- Phase 8.1.2 Inotia 1 future-proof save-length fix.

## Phase 8.5 result

The four PIC/GOT destinations written by `0x143ADC` are all valid:

- version target `0x194BF0`
- count target `0x194BF2`
- handle target `0x1918F0`
- array target `0x194BEC`

The guest still faults at address 0 immediately after the 55-byte i_pack
header is parsed. Therefore the fault is downstream of those stores.

## Next caller

`0x143A88` returns into `0x144E58`. That caller immediately dereferences three
additional GOT-resolved globals:

- GOT+0x03B8: u16 validation count
- GOT+0x03BC: u8 record stride
- GOT+0x03B4: u32 record-base pointer

Phase 8.6 records:
- each GOT destination;
- the typed value stored at that destination;
- the first 32 bytes at `record_base + 6` when mapped;
- a simple `null_stage` classification;
- a signature of the immediate `0x144E6A` caller block.

Markers:

    [PHASE8_6]
    [INOTIA2_VALIDATE_POST]

This is observational only. No guest memory or control flow is changed.
