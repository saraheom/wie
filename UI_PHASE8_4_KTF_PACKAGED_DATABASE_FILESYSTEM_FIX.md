# Phase 8.4 — KTF packaged-database filesystem fallback fix

This phase preserves the Inotia 1 future-proof save fix and the current
Inotia 2 diagnostics.

## Root cause identified

The Phase 8.3.1 test log shows:

    OPEN_REQUEST name=i_pack.dat mode=1 exists=false packaged_len=0
    OPEN_RESULT name=i_pack.dat mode=1 -> -12 (NOENT)

But the original KTF Inotia 2 archive physically contains:

    p/i_pack.dat

with a size of 1,489,150 bytes.

KTF `WIPICContext::get_resource_size` / `read_resource` previously queried
only the Java class loader. KTF archive-side files under P/ or p/ are exposed
through `System::filesystem()`, so database resources that are not duplicated
inside the JAR were invisible to `MC_dbOpenDataBase`.

## Fix

For KTF only:

1. Keep Java class-loader resources as the first choice.
2. If absent, fall back to the emulator filesystem.
3. Resolve, in order:
   - `<name>`
   - `P/<name>`
   - `p/<name>`
4. Use the exact same resolution order for both size and data reads.

This is generic KTF package-resource behavior, not an Inotia-specific binary
bypass.

## Expected Inotia 2 log

    [KTF_RESOURCE_FALLBACK] size name=i_pack.dat path=p/i_pack.dat size=1489150
    [KTF_RESOURCE_FALLBACK] read name=i_pack.dat path=p/i_pack.dat expected=1489150 read=1489150
    [INOTIA2_DB] OPEN_REQUEST name=i_pack.dat mode=1 ... packaged_len=1489150
    [INOTIA2_DB] OPEN_RESULT name=i_pack.dat mode=1 ... initial_len=1489150

If startup then advances beyond the old `메모리에러` screen, this confirms
the missing KTF archive-resource fallback as the compatibility defect.

No network behavior and no MapleStory-specific code are changed.
