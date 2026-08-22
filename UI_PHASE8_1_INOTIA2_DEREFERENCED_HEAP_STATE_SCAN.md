# Phase 8.1 — Inotia 2 dereferenced internal-heap state scan

Phase 8.0 reached the correct KTF Inotia 2 binary revision, but its reported
`capacity`, `used`, `free_head`, and related fields were still GOT/global
addresses rather than the values stored in those globals.

The tell was the Phase 8.0 output itself:

    descriptor_limit=0x001918e0
    capacity=0x001918e4
    ...
    used=0x00194b28

Those are pointer-like addresses, and static disassembly of allocator
`0x125C54` confirms that Inotia dereferences them before using them.

Phase 8.1 follows those indirections and also walks the allocator's two
descriptor chains.

## New information logged

`[INOTIA2_HEAP]` now reports the real:

- descriptor limit
- pool capacity
- used bytes / free bytes
- pool base
- free-descriptor head
- allocated-block head
- allocation count
- UI allocation result

It also scans the descriptor table and reports:

- number of free descriptors
- number of allocated descriptors
- whether each linked list is structurally valid
- sum of allocated sizes
- whether allocated blocks are address-sorted
- largest free gap in the 0xFA000-byte pool
- whether a 0x100-byte request should fit by capacity, descriptor availability,
  and contiguous-gap availability
- a compact sample of the first allocated descriptors

## Test

Use the original KTF Inotia 2 archive first:

- AID `010100D5`
- PID `PD007974`
- native `client.bin1149832`

Launch until the `메모리에러 / OK: 종료` screen appears, then export the global
diagnostic log.

The decisive line is:

    [INOTIA2_HEAP] chains ...

Interpretation:

- `capacity_can_fit=false` -> true pool exhaustion
- `descriptor_can_fit=false` -> descriptor/free-list exhaustion
- `gap_can_fit=false` with free capacity -> fragmentation/bookkeeping problem
- all three true while UI allocation remains 0 -> allocator traversal/list
  corruption, and the sampled descriptors will identify where it diverges

No guest memory or allocator state is modified in this phase.
