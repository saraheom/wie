# UI / Runtime Phase 8.25

- Inotia 1: persist `MC_netSetReadCB` registration and asynchronously wake the
  original guest callback when a local protocol response becomes available.
- Inotia 1: preserves the Phase 8.24 command-2/4/30 probes; catalog remains empty
  until the next authentic record format is recovered.
- Inotia 2: accelerate the exact measured RGB565 software-effect inner row at
  guest `0x00123f2a` using bulk guest-memory transfers and a Rust channel-LUT
  loop.
- Inotia 2: runtime mask/width safety gate with automatic restoration of the
  original guest instruction on mismatch.
- Preserve the 4,000-instruction execution slice, safe installer execution,
  hidden install-progress renderer, and all earlier auth/resource fixes.
