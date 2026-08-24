# WIPI Player Phase 8.31 — Inotia 1 three-page cash catalog + character-name recovery probe

Phase 8.31 is a full-repository release built directly on Phase 8.30.

## Field evidence from Phase 8.30

The August 24 field run validates the two Phase 8.30 special-item paths:

- `자원 교환권` reaches authentic outbound command 89 (`00 07 59 00 02 00 0a`),
  receives the local success frame (`00 04 59 01`), and then returns to the
  title's original inventory/save path.
- `축복받은 용사의 인장` works after the exact type-`0xBA` Network Mode gate is
  bypassed. No new host-side inventory effect is introduced.

The same run also provides a clean/corrupted save comparison in the live title:

- the uncorrupted August 22 backup loads `save0.dat` at 1480 bytes;
- after restoring the August 24 backup, `save0.dat` loads at 1560 bytes.

The static `char.dat` resource remains unchanged, so the persisted name
corruption is carried through the active save state rather than the character
definition database.

## Inotia 1 — correct native page indices 0 / 1 / 2

Phase 8.29 prevented the 18-record catalog overflow by publishing the entries as
12 + 6 records. That fixed new character-name corruption, but it assumed that
the command-30 final byte selected only page 0 or page 1.

Phase 8.30 field testing proves that assumption is incomplete. The guest emits
command-30 requests for page index 2 as well. In Phase 8.29/8.30, any request
other than `1` was collapsed to page 0, so the title could move from the initial
shop page to page 1 but could not display its native third page correctly.

Phase 8.31 keeps the same 18 zero-cost catalog definitions but publishes them as
three six-record pages:

- page 0: records 1–6;
- page 1: records 7–12;
- page 2: records 13–18.

Every response is therefore well below the previously observed fixed-array
overflow threshold, while the exact requested page index `0`, `1`, or `2` is
preserved.

Markers:

- `[PHASE8_31_INOTIA1_FIRST_OPEN_THREE_PAGE]`
- `[PHASE8_31_INOTIA1_CASH_THREE_PAGE]`

Expected navigation is page 0 -> page 1 -> page 2, with the opposite arrow able
to request the earlier page again according to the title's own command-30
requests.

## Inotia 1 — non-destructive character-name render probe

Phase 8.31 does not rewrite `save0.dat` and does not guess the original names.

Instead, the exact Inotia 1 graphics path logs each unique short rendered Hangul
string together with its guest pointer, coordinates, byte length, encoding, and
raw bytes:

`[PHASE8_31_INOTIA1_NAME_RENDER_PROBE]`

The probe is diagnostic only. A pointer/content pair is deduplicated to avoid
logging the same static label every frame; it does not change guest memory or
rendered text.

### Name-recovery test

1. Restore/load the August 22 uncorrupted backup.
2. Open the screen that visibly shows the character/scenario names and leave it
   visible briefly.
3. Return to the library.
4. Restore/load the August 24 corrupted backup.
5. Open the exact same screen and leave it visible briefly.
6. Export the diagnostic log.

Because the same title and UI path are exercised twice in one app session, the
probe should expose the clean and corrupted rendered name bytes/pointers
directly. That gives the next phase enough information for a targeted repair
without rolling back unrelated August 24 progress.

## Preserved Phase 8.30 behavior

Phase 8.31 intentionally keeps unchanged:

- the Phase 8.30 command-89 resource-exchange success bridge;
- the Phase 8.30 blessed-seal type-`0xBA` use-gate bypass;
- all earlier Inotia 1 handshake, purchase, callback, and network-use fixes;
- the Phase 8.30 Inotia 2 startup/main-menu diagnostic and existing performance
  paths.

No historical server is contacted.
