# UI Phase 5A — Library Polish and Portable Game Entries

Phase 5A builds on the working Phase 4.3 app-global diagnostics and keeps the existing
emulation, controls, save manager, TestFlight pipeline, and iOS orientation behavior intact.

## Library polish

- Favorite/unfavorite games directly from each cover.
- Library sorting:
  - Recently Played
  - Name
  - Favorites First
- Game count on the home screen.
- Per-game badges showing the saved orientation and display-size mode.
- Home/library Settings button.

## Home Settings

The global home settings screen now contains:

- Library sort preference.
- Default orientation for newly imported games.
- Default display-size mode for newly imported games.
- Language location prepared for Phase 7.

Existing games keep their own per-game orientation/display/control settings.

## Edit Game

The game action menu now includes **Edit Game**.

It supports:

- changing the library display name
- selecting/changing custom cover art

Display, controls, and save management remain separate per-game settings.

## Portable WIPI Player game entries

Each library game can be exported as:

`<game-name>.wipigame.json`

The portable entry contains:

- original ZIP/JAR package bytes
- WIPI Player display name
- custom cover image
- favorite state
- per-game orientation/display settings
- custom portrait/landscape control layouts

A portable entry can be imported from the home screen using **Import WIPI Entry**.

Normal in-game save data is intentionally NOT bundled into the portable game entry.
Use the existing Save Manager's `.wipisave.json` export/import for progress backups.
This separation prevents importing a game package from unexpectedly replacing progress.

When a portable entry replaces an existing game in the same library, the existing WIE
save-source association is preserved.

## Phase 7 preparation

The home Settings screen now has the permanent location for:

Settings → Language

Phase 5A exposes English only. Phase 7 can add 한국어 through the same settings screen
without restructuring the home UI.

## Build validation

The repo was checked for DOM ID consistency between the TypeScript handlers and HTML.
The local environment does not contain the repo's npm/webpack dependencies, so the
GitHub Windows smoke workflow remains the definitive production bundling test.
