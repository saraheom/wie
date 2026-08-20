# Phase 8.0 — Inotia 2 KTF internal heap probe

This phase starts the Inotia 2 work again from the working Phase 7.21
baseline. The Inotia 1 record-length fix is preserved unchanged.

## New static finding

The KTF Inotia 2 binary (`client.bin1149832`) reaches the visible
`메모리에러` screen because guest function `0x1450BC` requests 0x100
(256) bytes from the game's own allocator at `0x125C54`, and that allocator
returns NULL.

The allocator is initialized around `0x125BDC` with a nominal capacity of
`0xFA000` (1,024,000 bytes). Inotia 2 also explicitly requests
`WIPICX_incMemInterface`, so the next question is whether the game's internal
pool is actually exhausted or whether its free-list/backing state is invalid.

## What Phase 8.0 logs

On KTF Inotia 2 PID `PD007974`, when the game calls `MC_dbListDataBase`, it
reads (without modifying) the identified allocator globals:

- capacity / used / computed free bytes
- heap base / backing source
- block-table pointer
- free-list head
- secondary list/index
- allocation count
- UI allocation pointer that is NULL on the memory-error path
- ARM SP/LR/PC/CPSR
- code signatures at the allocator and caller

Markers:

    [PHASE8_0]
    [INOTIA2_HEAP]

## Test

Use the KTF archive first (`이노티아연대기2`, AID 010100D5 / PID PD007974).

1. Launch the game.
2. Wait until the `메모리에러` / `OK: 종료` screen appears.
3. Export the global diagnostic log.
4. Send the log back.

No save data, guest memory, WIPICX behavior, or allocator limits are changed
in this phase.

The result will distinguish among:

- true exhaustion (`used` approximately equals `capacity`);
- free-list/bookkeeping exhaustion despite free capacity;
- invalid/missing heap backing state.

That lets the next phase implement the actual WIPICX/internal-heap fix rather
than another guessed binary bypass.
