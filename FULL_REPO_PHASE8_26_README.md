# WIPI Player Phase 8.26 — Inotia 1 real offline catalog/purchase + Inotia 2 LZMA fast path

Phase 8.26 is a complete repository release built on Phase 8.25.

## Inotia 1 — first real catalog record and BUY response

Phase 8.25 proved the asynchronous KTF read wake is correct. The command-30
response is now consumed immediately and the original `캐쉬템 구매` screen opens.
The remaining blank list was expected because Phase 8.25 deliberately returned a
zero-record command-30 catalog.

Static analysis of the exact Inotia 1 native client resolves command 30 as:

```
u8 fixed0
u8 fixed1
u8 record_count
repeat record_count times:
    u8 name_len
    u8 name[name_len]
    u8 record_field
    u32 value
```

The user's first BUY attempt also exposed the authentic command-31 request and
its item-name bytes `B4 DC B0 CB`, EUC-KR `단검`.

Phase 8.26 therefore returns one real local record:

- item name: `단검`
- record count: 1
- record field: 1
- value/price: 0

Command 31 now receives common result/state `1` plus the four-byte zero value
consumed by the original purchase-success handler. The emulator still does not
write `char.dat` or inventory data directly: the original game success path is
responsible for any item grant/save mutation.

Markers:

- `[PHASE8_26_INOTIA1_CASH_CATALOG]`
- `[PHASE8_26_INOTIA1_CASH_PURCHASE]`
- existing `[PHASE8_25_INOTIA1_CASH_READ_WAKE]`

This is intentionally a one-item protocol proof. If the record renders and BUY
completes, later phases can expand the local catalog with more known item names
and zero prices without changing the transport model.

## Inotia 2 — host LZMA1 resource decompression

Phase 8.25's RGB565 effect fast path is preserved unchanged because field testing
showed that normal gameplay, skills, movement, and in-game menus became smooth.

The remaining startup black interval is CPU-bound. The field log still shows
roughly 65.5 million interpreted guest instructions inside the title's resource
decoder, while the surrounding persistent writes take only milliseconds.

Exact static analysis resolves guest `0x00125928` as the high-level LZMA wrapper:

```
r0 = compressed blob pointer
r1 = compressed blob length
r2 = caller-allocated output buffer
r3 = expected output length
return r3 on success / -1 on failure
```

The title uses a one-byte private prefix followed by a standard 13-byte
LZMA-Alone header. Multiple blobs from the exact packaged Inotia 2 resources
were independently decoded by skipping that first byte, with output lengths
matching their declared lengths.

Phase 8.26 installs an exact-title SVC hook at guest `0x00125928` and performs the
same decompression with pure-Rust `lzma-rs` instead of interpreting the range
decoder instruction by instruction. It preserves the caller's already allocated
output buffer and returns exactly the expected unpacked length.

Safety gates:

- exact Inotia 2 AID/PID/native length gate inherited from `load_native`;
- expected original first instruction bytes `F0 B5`;
- compressed length 14 bytes .. 16 MiB;
- output length 1 byte .. 32 MiB;
- LZMA properties byte <= `E0`;
- dictionary <= 32 MiB;
- declared unpacked size must equal the guest ABI length (or be unknown-size
  `FFFFFFFFFFFFFFFF`);
- output length must exactly match after host decode;
- guest input/output memory must be readable/writable.

If any runtime check fails, Phase 8.26 restores `F0 B5` and jumps back to the
original guest decoder for the rest of that launch.

Markers:

- `[PHASE8_26_INOTIA2_LZMA_FASTPATH] installed ...`
- `[PHASE8_26_INOTIA2_LZMA_FASTPATH] first decode accelerated ...`
- fallback, if needed: `[PHASE8_26_INOTIA2_LZMA_FASTPATH] runtime gate failed ...`

The unsafe Phase 8.17 initializer bypass remains absent. Required Inotia 2
initialization still runs; Phase 8.22 continues to hide only the obsolete visual
installation/progress renderer.

## Build scope

- Phase 8.25 RGB565 gameplay acceleration remains enabled.
- Inotia 2 scheduler remains at the known-working 4,000-instruction slice.
- Phase 8.23 per-frame profiler remains disabled.
- No intentional MapleStory or Heroes Lore 2 compatibility behavior changes.
