# Phase 7.3 — Inotia 2 KTF incremental-memory ABI probe

Phase 7.2 proved that Inotia 2 receives a non-null `WIPICX_incMemInterface` but does not call slot 0. This build exposes eight independently traced function-table slots.

Each slot logs its index and ARM r0-r3 arguments. A guarded allocator only allocates when one of those arguments looks like a sane byte-count (16 bytes through 32 MiB); pointer-like/free-style calls return 0.

This is an evidence-driven compatibility probe for an undocumented KTF extension, not a claim that all slots historically had identical semantics.
