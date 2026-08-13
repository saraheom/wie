# Phase 4.2 — iOS touch confirmation + landscape control editor

- Confirmation buttons now resolve on `pointerup` directly, with `click` retained as an accessibility/keyboard fallback.
- Confirmation interactions are logged as `confirmation settled: confirm` or `confirmation settled: cancel`.
- Confirmation overlay/modal/buttons explicitly own pointer hit testing above the emulator surface.
- In landscape, the control editor automatically docks opposite the pad being edited: D-pad selected -> editor on right; Number pad selected -> editor on left. This keeps the selected pad visible while changing size, spacing, opacity, visibility and position.
