# WIPI Player Phase 8.49 — global Inotia1 monster reward overflow repair

Phase 8.49 is based directly on Phase 8.48. It preserves the exact player EXP watchpoint, entity spawn trace, established Inotia1/Inotia2 compatibility behavior, save/revival paths, and the confirmed 10-record offline cash catalog with `힘의 조각` and `마법의 가지`.

## Root cause confirmed by Phase 8.48

The monster constructor calls a reward helper at guest `0x001281ec`. Its six effective inputs are `a=r0`, `b=r1` (unused by the original helper), `c=r2`, `d=r3`, `e=[sp]`, and `f=[sp+4]`. The helper performs ordinary 32-bit Thumb `MULS` operations followed by signed integer division.

Reconstructed formula:

```text
m = 100 - f
x = d + 100*a + 1000
y = 2*(50*a + d) + 1000
numerator   = c*m*x + f*y + e*m*y
denominator = m*y
reward      = numerator / denominator
```

In the original guest code, every multiply/add wraps to 32 bits before the signed divide. For the Phase 8.48 captures:

```text
수호자 C44: a=38, c=3690, d=960, e=10, f=3
wide numerator = 2,068,215,360
original reward = 3172
wide reward     = 3172

수호물 K34: a=38, c=4132, d=960, e=12, f=3
wide numerator = 2,316,473,280
wrapped signed numerator = -1,978,494,016
original reward = -3035
wide reward     = 3553
```

## Phase 8.49 repair behavior

A hash-scoped binary hook replaces the helper entry at Thumb PC `0x001281ed`. The repair is applied only for the verified monster-constructor caller `LR=0x00126245`; other callers receive the faithfully reconstructed original wrapped result.

For every monster-constructor call, Phase 8.49 computes:

1. the exact original 32-bit wrapped/signed result; and
2. the same formula with wide non-wrapping intermediates.

If the two results are identical, the monster is left unchanged. If they differ, the wide result is substituted. Therefore the repair is global across all monsters that use this constructor path, including future monsters whose overflow wraps negative or wraps again into an erroneously small positive reward.

No monster IDs, names, levels, entity slots, or hard-coded EXP values are used by the repair.

As an offline cross-check, the reconstructed original formula was evaluated against all 43 entity reward writes in the Phase 8.48 map-transition log and reproduced every logged original reward. The wide formula leaves the non-overflow families (`3172`, `2413`, `1734`, `1658`, `1`) unchanged and corrects the observed overflow families from `-3035 -> 3553` and `-2084 -> 4116`.

## Logging

Automatic startup marker:

```text
PHASE8_49_INOTIA1_REWARD_REPAIR_ACTIVE
```

Every constructor call through the helper logs:

```text
PHASE8_49_INOTIA1_REWARD_WIDE_MATH
```

Only corrected calls additionally log:

```text
PHASE8_49_INOTIA1_REWARD_OVERFLOW_REPAIR
```

The existing Arm/Reset EXP + Spawn Trace diagnostic remains optional. The repair is active even when that trace is disarmed.

## Recommended verification

First verify the same known pair:

1. load the same save;
2. optionally press **Arm/Reset EXP + Spawn Trace** so the exact player EXP change is logged;
3. kill `수호자 C44`;
4. kill `수호물 K34`;
5. export the log and, if convenient, record the displayed EXP values.

Expected behavior:

- `수호자 C44` should remain on the original non-overflowing reward path (`3172` base reward, no overflow-repair marker);
- `수호물 K34` should receive corrected base reward `3553` instead of `-3035`, producing positive final player EXP;
- other overflowing monsters should be corrected by the same formula automatically.

After that pair is confirmed, test `부서진 골렘`, `마력의 골렘`, `마력의 생물`, or other monsters that previously produced negative or suspiciously small EXP. No additional per-monster patch should be necessary if they use the same constructor path.
