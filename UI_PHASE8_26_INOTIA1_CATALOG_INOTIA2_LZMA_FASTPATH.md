# UI / Runtime Phase 8.26

- **Inotia 1:** command-30 now returns one actual local/free `단검` catalog record instead of zero records.
- **Inotia 1:** command-31 BUY receives an async local success response and continues through the original game purchase handler.
- **Inotia 2:** preserves the Phase 8.25 RGB565 gameplay fast path.
- **Inotia 2:** accelerates the exact guest LZMA wrapper at `0x00125928` with guarded host decompression to target black startup/resource-decode time and any menu asset decode using the same wrapper.
- The required Inotia 2 initializer is still executed; only its obsolete progress UI remains hidden.
