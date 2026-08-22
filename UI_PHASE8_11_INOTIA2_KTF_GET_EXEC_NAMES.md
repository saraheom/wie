# Phase 8.11 — Inotia 2 KTF `MC_knlGetExecNames` compatibility

## What the Phase 8.10 test proved

Phase 8.10 successfully returned the legacy access mask `0xBC`. The game then
advanced past error 1001 and immediately called kernel slot 2,
`MC_knlGetExecNames`. WIE still routed that slot to a fatal `Unimplemented`
stub, which terminated the game thread.

## Reconstructed call contract for this title

Disassembly of the exact `PD007974` / `010100D5` `client.bin` shows the caller
invokes slot 2 with five arguments:

```
MC_knlGetExecNames("010100D5", NULL, NULL, out_buf, 300)
```

The caller requires a positive return value, runs `strlen(out_buf)`, subtracts
21, then compares two 8-byte fields separated by one byte. Therefore its
expected first entry has the shape:

```
AAAAAAAA?AAAAAAAA????
```

The canonical executable name for this archive is exactly 21 bytes and fits
that contract:

```
010100D5/010100D5.jar
```

The JAR in the supplied KTF archive is also named `010100D5.jar`.

## Compatibility behavior

For only AID `010100D5`, PID `PD007974`:

- match the query program name `010100D5`;
- require null version and vendor filters (as this caller supplies);
- write `010100D5/010100D5.jar\0\0` to the 300-byte result buffer;
- return `1` (one match);
- log `[PHASE8_11_EXECNAMES]` once.

All other titles retain the prior unimplemented behavior so this experiment
cannot change MapleStory or unrelated WIPI games.
