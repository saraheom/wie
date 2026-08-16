# Phase 7.8 — Inotia 2 Storage Compatibility

Phase 7.8 follows the successful Phase 7.7 MapleStory compatibility work.

## What the Phase 7.7 log proved

The LGT Inotia 2 build now reaches WIPIC service 0x19C, which Phase 7.7 maps to
the WIPI database/storage query. Immediately afterward the game formats:

    설치(또는 실행) 공간이 부족합니다. 2103KB의 저장공간이 필요합니다.

("There is not enough install/run storage. 2103 KB of storage is required.")

The WIE database layer was still exposing only a 1 MiB virtual handset storage
quota, which is below Inotia 2's explicit 2103 KiB requirement.

## Phase 7.8 change

The virtual WIPI persistent-storage quota is raised from:

    1 MiB

to:

    16 MiB

The value returned to games is still `quota - actual database usage`, so this
does not bypass WIE persistence or fake an unlimited value.

Every storage query now emits a diagnostic line:

    [DB_STORAGE] MC_dbListDataBase available=... used=... limit=16777216

## MapleStory logging

MapleStory is confirmed to continue and save successfully after the 0x416
`memcmp` and 0xCF graphics-context fixes. Per-call successful `memcmp` logging
is therefore demoted from INFO to DEBUG to prevent thousands of redundant lines
from crowding the app diagnostic log.

## KTF Inotia 2

The original/flattened KTF Inotia 2 remains a separate issue. Those builds
initialize, create their persistence records, and enter their second runtime
thread without a hard WIE exception. Phase 7.8 does not add another speculative
KTF memory patch.
