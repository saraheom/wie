# Phase 8.5 — Inotia 2 i_pack post-header global-pointer probe

Phase 8.4 fixed KTF packaged-database lookup: `p/i_pack.dat` is now found,
read in full, persisted, opened, and parsed.

The KTF build then deterministically faults with `Invalid memory access; address: 0`
immediately after the 55-byte i_pack header is consumed. The parser at guest
`0x143A88` has just read:

- version: 1 byte
- count: 2 bytes (`13`)
- 13 offsets: 52 bytes

Total: 55 bytes.

Static disassembly shows that the very next code (`0x143ADC..0x143AFA`) stores
the parsed version, count, DB handle, and allocated offset-array pointer through
four PIC/GOT-resolved global pointers. A zero or invalid GOT destination would
explain the immediate null-address fault.

Phase 8.5 adds a title-scoped, observational checkpoint on the final i_pack
header read. It logs:

- GOT slots at `0x193AD4`, `0x193AD8`, `0x193ACC`, `0x193AD0`;
- each resolved destination pointer;
- a 32-bit probe at each nonzero destination;
- code words around `0x143ADC`;
- full ARM registers and 64 bytes of stack.

Markers:

    [PHASE8_5]
    [INOTIA2_IPACK_POST]

No guest registers, memory, database bytes, or return values are changed.
The Phase 8.4 filesystem fallback and the Inotia 1 save-length fix remain
present.
