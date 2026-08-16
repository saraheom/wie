# Phase 7.4A — Inotia 2 Native Memory Trace

This phase intentionally stops guessing at the undocumented KTF incremental-memory ABI.
It is a diagnostic build designed to identify the actual failure boundary in Inotia 2.

## What Phase 7.3 established

- The original and flattened Inotia 2 archives both load as KTF application `010100D5 / PD007974`.
- The ARM native thread starts and persistent records are initialized.
- Returning a non-null `WIPICX_incMemInterface` does not remove the game's memory-error screen.
- An eight-slot function table is also not called by the game.
- WIE's general ARM allocator is already 256 MiB at `0x40000000`, so the error is not a global emulator-memory shortage.

## Diagnostic experiment 1: contiguous native-image headroom

KTF loads `client.bin` at `0x00100000` and historically mapped only:

`client.bin data + BSS declared in the client.bin filename`

For titles whose declared BSS is at least 512 KiB, Phase 7.4A maps an additional **8 MiB of contiguous writable headroom** after the nominal code/data+BSS area.

The declared BSS value passed to the guest relocation/bootstrap function is **not changed**. This means the experiment does not lie to the game about its BSS size; it only tests whether the old KTF C runtime expects writable address space beyond the nominal BSS boundary for `_sbrk`, `new`, or `malloc`.

Inotia 1 has a much smaller BSS and does not receive this experimental headroom, so it remains a useful control.

Expected log:

```
[KTF_MEM] native image file=client.bin1149832 ... bss=... headroom=0x800000 ...
```

### Interpretation

- If Inotia 2 boots: the failure is strongly consistent with native contiguous heap exhaustion / mapping.
- If the memory-error screen is unchanged: the runtime is probably making an explicit configuration/interface check instead of simply touching unmapped memory.

## Diagnostic experiment 2: exact getInterface caller tracing

Every KTF `getInterface()` request now records:

- requested interface name
- SVC PC
- guest LR / normalized caller address
- 32 bytes of guest code around that return address
- returned interface pointer

Example:

```
[KTF_IFACE] request=WIPICX_incMemInterface svc_pc=... caller_lr=... caller=...
[KTF_IFACE] caller_words base=... [...]
[KTF_IFACE] response=WIPICX_incMemInterface ptr=... caller=...
```

The caller address can be matched directly against a relocated image produced by the existing `wie_ktf_dump` utility. This lets us determine whether the guest subsequently treats the returned pointer as:

- a function table,
- a structure containing base/size fields,
- a versioned descriptor,
- or something else.

## Diagnostic experiment 3: KTF initialization layout

The build records the native image/BSS boundaries and the addresses supplied to the undocumented KTF initialization parameters (`InitParam0`, `InitParam1`, JVM context, `InitParam3`, and `InitParam4`). This prepares the next comparison between Inotia 1 and Inotia 2 if the headroom experiment does not change behavior.

## Test procedure

1. Install this TestFlight build.
2. Clear the global diagnostic log.
3. Import only the original Inotia 2 ZIP (the wrapper-folder normalization remains enabled).
4. Launch it once and wait until the memory-error screen appears, or until it boots.
5. Return to the library and export the global diagnostic log.
6. As a control, launch Inotia 1 once in the same session and export the log again if practical.

Do not use the older `Inotia2-WIE-Memory-Fix-Patch` package in this phase; it fails at archive/emulator creation before the native-runtime experiment can run.

## Build validation

The local execution environment used to prepare this repo does not include the Rust toolchain. GitHub's existing smoke/TestFlight workflow remains the compile-time validation step for these Rust changes.
