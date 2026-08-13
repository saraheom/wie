# UI Phase 2 — Orientation and Display Scaling

This phase adds per-game display settings while keeping the WIE emulation framebuffer unchanged.

## Added

- In-player rotate button.
- Per-game Portrait / Landscape preference.
- iOS support for portrait, landscape-left, and landscape-right orientations.
- Screen Orientation API request on supported WebKit versions, with a CSS-layout fallback.
- Per-game display size modes:
  - Original 240 × 320
  - Compact
  - Fit
  - Large
  - Maximum
- Display Settings shortcut from each game library card's `...` menu.
- Existing game records are normalized automatically so Phase 1 libraries remain compatible.

## Notes

"Screen size" changes the presentation size of the 240 × 320 game canvas. It does not change
the emulated WIPI framebuffer resolution, which avoids breaking games that assume the original
phone display dimensions.

Control-layout editing is intentionally reserved for Phase 3.
