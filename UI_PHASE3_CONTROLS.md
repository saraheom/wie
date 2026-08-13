# UI Phase 3 — Per-Game Control Layout Editor

Phase 3 adds a live virtual-control editor while preserving the working Phase 2.1
orientation, display, library, diagnostics, and TestFlight behavior.

## Per-game controls

Each library game now stores separate Portrait and Landscape control layouts.

Customizable properties:

- Drag the D-pad group anywhere on the player area.
- Drag the number-pad group independently.
- D-pad size.
- Number-pad size.
- Spacing between buttons.
- Key opacity.
- Show/hide any individual virtual key.

Starter layouts:

- Classic
- Spacious
- Compact

`Reset` restores the default positions/sizes for the orientation currently being edited.
Portrait and landscape settings do not overwrite each other.

## Editing

Open a running game's Settings and choose **Customize Controls**, or open a game's
`...` menu in the library and choose **Customize Controls**.

The actual running game remains visible while editing. During edit mode virtual key
presses are disabled so dragging a pad cannot accidentally send WIPI key events.

Changes are saved to the game's IndexedDB library record and persist across relaunches.

## Compatibility

Existing Phase 1 / Phase 2 game records are normalized automatically. No game re-import
should be required. The temporary Phase 2.1 diagnostic logger remains enabled and records
control-editor changes for testing.
