import { WieWeb } from "@pkg";
import { setMasterVolume } from "./midi";
import { clearDebugLog, debugLog, exportDebugLog, getDebugLogText, getDebugSessionId, installDebugLogging, setDebugGameScope } from "./debug_log";
import {
  defaultGameSettings,
  displayNameForFile,
  GameLibrary,
  GameRecord,
  gameIdForArchive,
} from "./game_library";
import {
  CONTROL_KEYS,
  ControlKey,
  ControlPadName,
  controlPresetLayout,
  defaultControlLayout,
} from "./control_layout";
import {
  createSaveBackup,
  deleteSaveBackup,
  eraseGameSaveData,
  exportSaveBackup,
  hasSaveSources,
  listSaveBackups,
  parseImportedSaveBackup,
  restoreSaveBackup,
  SaveBackup,
  storeImportedSaveBackup,
} from "./save_manager";
import { errorFeedback, initUiFeedback, requestConfirmation, successFeedback } from "./ui_feedback";
import { AppSettings, loadAppSettings, saveAppSettings } from "./app_settings";
import { exportGameEntry, parseGameEntry } from "./game_entry";
import { IndexedDBStore } from "./indexed_db_store";
import { applyTranslations, setLanguage, t } from "./i18n";

installDebugLogging();

const TUTORIAL_STORAGE_KEY = "wipi_player_tutorial_dismissed_v2";
const MIDI_VOLUME_STORAGE_KEY = "wipi_player_midi_volume";
const PCM_VOLUME_STORAGE_KEY = "wipi_player_pcm_volume";

const keyMap: Record<string, string> = {
  Digit1: "1",
  Digit2: "2",
  Digit3: "3",
  KeyQ: "4",
  KeyW: "5",
  KeyE: "6",
  KeyA: "7",
  KeyS: "8",
  KeyD: "9",
  KeyZ: "*",
  KeyX: "0",
  KeyC: "#",
  Backspace: "CLR",
  ArrowUp: "UP",
  ArrowLeft: "LEFT",
  ArrowRight: "RIGHT",
  ArrowDown: "DOWN",
  Space: "OK",
};

let library: GameLibrary;
let currentEmulator: WieWeb | undefined;
let currentGame: GameRecord | undefined;
let wakeLockSentinel: any | undefined;
let animationFrame: number | undefined;
let activeActionGameId: string | undefined;
let pendingCoverGameId: string | undefined;
let renderedCoverUrls: string[] = [];
const coverUrlCache = new Map<string, string>();
let controlEditing = false;
let selectedControlPad: ControlPadName = "direction";
let saveManagerGameId: string | undefined;
let appSettings: AppSettings = loadAppSettings();
setLanguage(appSettings.language);
let pendingEditGameId: string | undefined;

const element = <T extends HTMLElement>(id: string): T => {
  const found = document.getElementById(id);
  if (!found) throw new Error(`Missing element #${id}`);
  return found as T;
};

type OrientationLockType = "portrait-primary" | "landscape-primary";

const setOrientationStatus = (message: string) => {
  const status = document.getElementById("orientation-status");
  if (status) status.textContent = message;
};

const requestOrientation = async (orientation: "portrait" | "landscape") => {
  const screenOrientation = screen.orientation as ScreenOrientation & {
    lock?: (orientation: OrientationLockType) => Promise<void>;
    unlock?: () => void;
  };

  if (!screenOrientation?.lock) {
    setOrientationStatus("The game layout changed. Physical orientation locking is not available on this platform.");
    return;
  }

  try {
    await screenOrientation.lock(
      orientation === "landscape" ? "landscape-primary" : "portrait-primary"
    );
    setOrientationStatus(
      orientation === "landscape"
        ? "Landscape mode is active for this game."
        : "Portrait mode is active for this game."
    );
  } catch (error) {
    console.warn("Could not lock device orientation", error);
    setOrientationStatus(
      "The game layout changed, but iOS did not rotate automatically. Rotate the phone manually if needed."
    );
  }
};

const unlockOrientation = () => {
  const screenOrientation = screen.orientation as ScreenOrientation & {
    unlock?: () => void;
  };
  try {
    screenOrientation?.unlock?.();
  } catch (error) {
    console.warn("Could not unlock orientation", error);
  }
};

const forcePlayerLayoutRefresh = () => {
  const player = document.getElementById("player-view");
  const content = document.querySelector<HTMLElement>("#player-view .player-content");
  if (!player || !content || player.hidden) return;

  // WKWebView can delay recomputing a display:contents/grid transition while the
  // device rotation animation is running. Rebuild this layout box synchronously.
  const previousDisplay = content.style.display;
  content.style.display = "none";
  void content.offsetHeight;
  content.style.display = previousDisplay;
  void content.offsetHeight;

  requestAnimationFrame(() => {
    void content.offsetWidth;
  });
};

let viewportRefreshFrame: number | undefined;
const schedulePlayerLayoutRefresh = (source: string) => {
  if (!currentGame) return;
  if (viewportRefreshFrame !== undefined) cancelAnimationFrame(viewportRefreshFrame);
  viewportRefreshFrame = requestAnimationFrame(() => {
    viewportRefreshFrame = undefined;
    forcePlayerLayoutRefresh();
    debugLog(
      "LAYOUT",
      `refresh source=${source}`,
      `orientation=${currentGame?.settings.orientation}`,
      `display=${currentGame?.settings.displayMode}`,
      `viewport=${window.innerWidth}x${window.innerHeight}`
    );
  });
};

const applyGameDisplaySettings = (game: GameRecord, requestDeviceRotation = false) => {
  const player = element("player-view");
  player.dataset.orientation = game.settings.orientation;
  player.dataset.displayMode = game.settings.displayMode;

  element<HTMLSelectElement>("game-orientation").value = game.settings.orientation;
  element<HTMLSelectElement>("game-display-mode").value = game.settings.displayMode;

  element("rotate-screen").textContent = game.settings.orientation === "portrait" ? "↻" : "↺";
  element("rotate-screen").setAttribute(
    "aria-label",
    game.settings.orientation === "portrait" ? "Switch to landscape" : "Switch to portrait"
  );

  applyControlSettings(game);
  forcePlayerLayoutRefresh();
  debugLog(
    "DISPLAY",
    `apply orientation=${game.settings.orientation}`,
    `mode=${game.settings.displayMode}`,
    `requestDeviceRotation=${requestDeviceRotation}`
  );

  if (requestDeviceRotation) {
    void requestOrientation(game.settings.orientation).finally(() => {
      schedulePlayerLayoutRefresh("orientation-request-complete");
    });
  } else {
    setOrientationStatus(
      game.settings.orientation === "landscape" ? "Landscape layout" : "Portrait layout"
    );
  }
};

const saveCurrentGameSettings = async () => {
  if (!currentGame) return;
  await library.put(currentGame);
};

const currentControlLayout = () => {
  if (!currentGame) return undefined;
  return currentGame.settings.controlLayouts[currentGame.settings.orientation];
};

const applyControlSettings = (game: GameRecord) => {
  const player = element("player-view");
  const layout = game.settings.controlLayouts[game.settings.orientation];
  player.dataset.controls = game.settings.controlPreset;

  const direction = document.querySelector<HTMLElement>(".direction-pad");
  const number = document.querySelector<HTMLElement>(".number-pad");
  if (!direction || !number) return;

  const applyPad = (pad: HTMLElement, name: ControlPadName) => {
    const setting = layout[name];
    pad.style.setProperty("--control-x", `${setting.x}%`);
    pad.style.setProperty("--control-y", `${setting.y}%`);
    pad.style.setProperty("--control-scale", String(setting.scale));
    pad.style.setProperty("--control-gap", `${setting.gap}px`);
  };

  applyPad(direction, "direction");
  applyPad(number, "number");
  player.style.setProperty("--control-opacity", String(layout.opacity));

  for (const button of document.querySelectorAll<HTMLButtonElement>("button[data-key]")) {
    const key = button.dataset.key as ControlKey | undefined;
    button.hidden = !!key && layout.hiddenKeys.includes(key);
  }

  updateControlEditorUi();
  schedulePlayerLayoutRefresh("controls-apply");
};

const ensureCustomControls = () => {
  if (!currentGame) return;
  if (currentGame.settings.controlPreset === "custom") return;
  currentGame.settings.controlPreset = "custom";
  currentGame.settings.controlLayouts.portrait = defaultControlLayout("portrait");
  currentGame.settings.controlLayouts.landscape = defaultControlLayout("landscape");
};

const setSelectedControlPad = (pad: ControlPadName) => {
  selectedControlPad = pad;
  element("control-pad-direction").classList.toggle("active", pad === "direction");
  element("control-pad-number").classList.toggle("active", pad === "number");
  document.querySelector(".direction-pad")?.classList.toggle("control-selected", pad === "direction");
  document.querySelector(".number-pad")?.classList.toggle("control-selected", pad === "number");
  const editor = document.getElementById("control-editor");
  editor?.classList.toggle("editing-direction", pad === "direction");
  editor?.classList.toggle("editing-number", pad === "number");
  updateControlEditorUi();
};

const updateControlEditorUi = () => {
  if (!currentGame) return;
  const layout = currentControlLayout();
  if (!layout) return;

  const pad = layout[selectedControlPad];
  const size = document.getElementById("control-size") as HTMLInputElement | null;
  const gap = document.getElementById("control-gap") as HTMLInputElement | null;
  const opacity = document.getElementById("control-opacity") as HTMLInputElement | null;
  const orientation = document.getElementById("control-editor-orientation");
  const sizeValue = document.getElementById("control-size-value");
  const gapValue = document.getElementById("control-gap-value");
  const opacityValue = document.getElementById("control-opacity-value");

  if (size) size.value = String(Math.round(pad.scale * 100));
  if (gap) gap.value = String(Math.round(pad.gap));
  if (opacity) opacity.value = String(Math.round(layout.opacity * 100));
  if (orientation) orientation.textContent = currentGame.settings.orientation === "portrait" ? (appSettings.language === "ko" ? "세로 레이아웃" : "Portrait layout") : (appSettings.language === "ko" ? "가로 레이아웃" : "Landscape layout");
  if (sizeValue) sizeValue.textContent = `${Math.round(pad.scale * 100)}%`;
  if (gapValue) gapValue.textContent = `${Math.round(pad.gap)} px`;
  if (opacityValue) opacityValue.textContent = `${Math.round(layout.opacity * 100)}%`;

  for (const input of document.querySelectorAll<HTMLInputElement>("input[data-control-key]")) {
    const key = input.dataset.controlKey as ControlKey;
    input.checked = !layout.hiddenKeys.includes(key);
  }
};

const openControlEditor = async () => {
  if (!currentGame) return;
  ensureCustomControls();
  controlEditing = true;
  element("player-view").classList.add("control-editing");
  element("control-editor").hidden = false;
  element("settings-panel").classList.remove("visible");
  element("settings-panel").setAttribute("aria-hidden", "true");
  setSelectedControlPad("direction");
  applyControlSettings(currentGame);
  debugLog("CONTROLS", `editor opened orientation=${currentGame.settings.orientation}`);
  await saveCurrentGameSettings();
};

const closeControlEditor = async () => {
  controlEditing = false;
  element("player-view").classList.remove("control-editing");
  element("control-editor").hidden = true;
  document.querySelector(".direction-pad")?.classList.remove("control-selected");
  document.querySelector(".number-pad")?.classList.remove("control-selected");
  debugLog("CONTROLS", "editor closed");
  await saveCurrentGameSettings();
};

const applyControlStarter = async (preset: "classic" | "spacious" | "compact") => {
  if (!currentGame) return;
  currentGame.settings.controlPreset = "custom";
  currentGame.settings.controlLayouts[currentGame.settings.orientation] = controlPresetLayout(
    currentGame.settings.orientation,
    preset
  );
  applyControlSettings(currentGame);
  debugLog("CONTROLS", `starter=${preset} orientation=${currentGame.settings.orientation}`);
  await saveCurrentGameSettings();
};

const resetControlsForOrientation = async () => {
  if (!currentGame) return;
  currentGame.settings.controlPreset = "custom";
  currentGame.settings.controlLayouts[currentGame.settings.orientation] = defaultControlLayout(
    currentGame.settings.orientation
  );
  applyControlSettings(currentGame);
  debugLog("CONTROLS", `reset orientation=${currentGame.settings.orientation}`);
  await saveCurrentGameSettings();
};

const showTutorial = () => {
  element("tutorial-overlay").hidden = false;
};

const hideTutorial = () => {
  const checkbox = element<HTMLInputElement>("dont-show-again");
  if (checkbox.checked) {
    localStorage.setItem(TUTORIAL_STORAGE_KEY, "true");
  }
  element("tutorial-overlay").hidden = true;
};

const initTutorial = () => {
  element("close-tutorial").addEventListener("click", hideTutorial);
  element("show-tutorial").addEventListener("click", showTutorial);
  element("tutorial-overlay").addEventListener("click", (event) => {
    if (event.target === event.currentTarget) hideTutorial();
  });

  if (localStorage.getItem(TUTORIAL_STORAGE_KEY) !== "true") {
    showTutorial();
  }
};

const restoreVolumeControls = () => {
  const midiSlider = element<HTMLInputElement>("volume-midi");
  const pcmSlider = element<HTMLInputElement>("volume-pcm");

  midiSlider.value = localStorage.getItem(MIDI_VOLUME_STORAGE_KEY) ?? "50";
  pcmSlider.value = localStorage.getItem(PCM_VOLUME_STORAGE_KEY) ?? "50";

  setMasterVolume(Number(midiSlider.value) / 100);

  midiSlider.addEventListener("input", () => {
    localStorage.setItem(MIDI_VOLUME_STORAGE_KEY, midiSlider.value);
    setMasterVolume(Number(midiSlider.value) / 100);
  });

  pcmSlider.addEventListener("input", () => {
    localStorage.setItem(PCM_VOLUME_STORAGE_KEY, pcmSlider.value);
    currentEmulator?.set_pcm_volume(Number(pcmSlider.value) / 100);
  });
};

const clearRenderedCoverUrls = () => {
  // Cover URLs are cached by game ID so lightweight library rerenders (favorite/sort)
  // do not revoke the image that WebKit is still displaying. They are explicitly
  // invalidated when a cover changes or a game is deleted.
  renderedCoverUrls = Array.from(coverUrlCache.values());
};

const invalidateCoverUrl = (gameId: string) => {
  const url = coverUrlCache.get(gameId);
  if (url) URL.revokeObjectURL(url);
  coverUrlCache.delete(gameId);
};

const makeCoverElement = (game: GameRecord): HTMLElement => {
  if (game.cover) {
    const image = document.createElement("img");
    let url = coverUrlCache.get(game.id);
    if (!url) {
      url = URL.createObjectURL(game.cover);
      coverUrlCache.set(game.id, url);
    }
    image.src = url;
    image.alt = `${game.name} cover`;
    image.className = "game-cover";
    image.addEventListener("error", () => {
      debugLog("LIBRARY", `cover image load error game=${game.name}`);
      invalidateCoverUrl(game.id);
    }, { once: true });
    return image;
  }

  invalidateCoverUrl(game.id);
  const placeholder = document.createElement("span");
  placeholder.className = "game-cover-placeholder";
  const normalized = game.name.trim();
  placeholder.textContent = normalized ? Array.from(normalized)[0].toUpperCase() : "G";
  return placeholder;
};

const renderLibrary = async () => {
  let games = await library.list();
  const grid = element("game-grid");
  const empty = element("library-empty");
  const count = document.getElementById("library-count");

  const direction = appSettings.librarySortDirection === "asc" ? 1 : -1;
  if (appSettings.librarySort === "name") {
    games = games.sort((a, b) => direction * a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
  } else if (appSettings.librarySort === "favorites") {
    games = games.sort((a, b) => {
      if (Boolean(a.favorite) !== Boolean(b.favorite)) {
        const favoriteOrder = a.favorite ? -1 : 1;
        return appSettings.librarySortDirection === "desc" ? favoriteOrder : -favoriteOrder;
      }
      const aTime = a.lastPlayedAt ?? a.createdAt;
      const bTime = b.lastPlayedAt ?? b.createdAt;
      return direction * (aTime - bTime);
    });
  } else {
    games = games.sort((a, b) => {
      const aTime = a.lastPlayedAt ?? a.createdAt;
      const bTime = b.lastPlayedAt ?? b.createdAt;
      return direction * (aTime - bTime);
    });
  }

  if (count) count.textContent = appSettings.language === "ko" ? `${games.length}${t(games.length === 1 ? "count.game" : "count.games")}` : `${games.length} ${t(games.length === 1 ? "count.game" : "count.games")}`;

  const liveGameIds = new Set(games.map((game) => game.id));
  for (const gameId of Array.from(coverUrlCache.keys())) {
    if (!liveGameIds.has(gameId)) invalidateCoverUrl(gameId);
  }
  clearRenderedCoverUrls();
  grid.replaceChildren();
  empty.hidden = games.length !== 0;

  for (const game of games) {
    const card = document.createElement("article");
    card.className = "game-card";
    card.dataset.gameId = game.id;

    const coverWrap = document.createElement("div");
    coverWrap.className = "game-cover-wrap";

    const coverButton = document.createElement("button");
    coverButton.type = "button";
    coverButton.className = "game-cover-button";
    coverButton.ariaLabel = `Play ${game.name}`;
    coverButton.appendChild(makeCoverElement(game));
    coverButton.addEventListener("click", () => void launchGame(game.id));

    const favoriteButton = document.createElement("button");
    favoriteButton.type = "button";
    favoriteButton.className = `game-favorite-button${game.favorite ? " active" : ""}`;
    favoriteButton.textContent = game.favorite ? "★" : "☆";
    favoriteButton.ariaLabel = game.favorite ? `Remove ${game.name} from favorites` : `Add ${game.name} to favorites`;
    let favoritePointerHandled = false;
    const toggleFavorite = async () => {
      const fresh = await library.get(game.id);
      if (!fresh) return;
      fresh.favorite = !fresh.favorite;
      await library.put(fresh);
      favoriteButton.textContent = fresh.favorite ? "★" : "☆";
      favoriteButton.classList.toggle("active", fresh.favorite);
      favoriteButton.ariaLabel = fresh.favorite ? `Remove ${fresh.name} from favorites` : `Add ${fresh.name} to favorites`;
      debugLog("LIBRARY", `favorite game=${fresh.name} value=${fresh.favorite}`);
      // Only reorder the whole grid when the active sort actually depends on favorites.
      if (appSettings.librarySort === "favorites") await renderLibrary();
    };
    favoriteButton.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      event.stopPropagation();
      favoritePointerHandled = true;
      try { favoriteButton.setPointerCapture(event.pointerId); } catch { /* optional on WebKit */ }
    });
    favoriteButton.addEventListener("pointerup", (event) => {
      event.preventDefault();
      event.stopPropagation();
      void toggleFavorite();
      window.setTimeout(() => { favoritePointerHandled = false; }, 350);
    });
    favoriteButton.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      if (!favoritePointerHandled) void toggleFavorite();
    });
    favoriteButton.addEventListener("contextmenu", (event) => event.preventDefault());

    coverWrap.append(coverButton, favoriteButton);

    const meta = document.createElement("div");
    meta.className = "game-meta";

    const name = document.createElement("span");
    name.className = "game-name";
    name.textContent = game.name;

    const fileName = document.createElement("span");
    fileName.className = "game-file";
    fileName.textContent = game.fileName;

    const badges = document.createElement("div");
    badges.className = "game-badges";
    const orientation = document.createElement("span");
    orientation.textContent = game.settings.orientation === "landscape" ? t("badge.landscape") : t("badge.portrait");
    const display = document.createElement("span");
    display.textContent = game.settings.displayMode === "native" ? "240×320" : game.settings.displayMode;
    badges.append(orientation, display);

    const menuButton = document.createElement("button");
    menuButton.type = "button";
    menuButton.className = "game-menu-button";
    menuButton.textContent = "⋯";
    menuButton.ariaLabel = `${game.name} options`;
    menuButton.addEventListener("click", () => openGameActions(game));

    meta.append(name, fileName, badges, menuButton);
    card.append(coverWrap, meta);
    grid.appendChild(card);
  }
};

const importGame = async (file: File) => {
  debugLog("LIBRARY", `import start file=${file.name}`, `size=${file.size}`);
  const data = await file.arrayBuffer();
  const id = await gameIdForArchive(data);
  const existing = await library.get(id);

  if (existing) {
    const shouldOpen = await requestConfirmation({
      title: "Already Imported",
      message: `“${existing.name}” is already in My Games. Open it now?`,
      confirmLabel: "Open Game",
    });
    if (shouldOpen) await launchGame(id);
    return;
  }

  const game: GameRecord = {
    id,
    name: displayNameForFile(file.name),
    fileName: file.name,
    archive: data,
    createdAt: Date.now(),
    settings: { ...defaultGameSettings(), orientation: appSettings.defaultOrientation, displayMode: appSettings.defaultDisplayMode },
    saveSources: { databases: [], filesystemAids: [] },
  };

  await library.put(game);
  debugLog("LIBRARY", `import complete game=${game.name}`, `id=${game.id}`);
  await renderLibrary();
  debugLog("BOOT", "WIPI Player ready on library view");
};

const releaseWakeLock = async () => {
  if (!wakeLockSentinel) return;
  const sentinel = wakeLockSentinel;
  wakeLockSentinel = undefined;
  try {
    await sentinel.release?.();
    debugLog("MOBILE", "screen wake lock released");
  } catch (error) {
    debugLog("MOBILE", "screen wake lock release failed", error);
  }
};

const requestWakeLockForPlayer = async () => {
  if (!appSettings.keepScreenAwake || document.visibilityState !== "visible" || !currentGame) return;
  const wakeLock = (navigator as any).wakeLock;
  if (!wakeLock?.request) {
    debugLog("MOBILE", "Screen Wake Lock API unavailable");
    return;
  }
  if (wakeLockSentinel) return;
  try {
    wakeLockSentinel = await wakeLock.request("screen");
    wakeLockSentinel?.addEventListener?.("release", () => {
      wakeLockSentinel = undefined;
    }, { once: true });
    debugLog("MOBILE", "screen wake lock acquired");
  } catch (error) {
    debugLog("MOBILE", "screen wake lock request failed", error);
  }
};

const stopCurrentGame = async () => {
  controlEditing = false;
  document.getElementById("player-view")?.classList.remove("control-editing");
  const controlEditor = document.getElementById("control-editor");
  if (controlEditor) controlEditor.hidden = true;
  if (animationFrame !== undefined) {
    cancelAnimationFrame(animationFrame);
    animationFrame = undefined;
  }

  if (currentEmulator) {
    const pending = IndexedDBStore.getPendingWriteCount();
    if (pending > 0) debugLog("SAVE_IO", `waiting for ${pending} pending write(s) before emulator shutdown`);
    try {
      await IndexedDBStore.flushPendingWrites();
      if (pending > 0) debugLog("SAVE_IO", "pending writes committed before emulator shutdown");
    } catch (error) {
      console.error("Failed while waiting for save writes", error);
    }

    try {
      currentEmulator.free();
    } catch (error) {
      console.warn("Failed to free emulator cleanly", error);
    }
  }

  currentEmulator = undefined;
  await releaseWakeLock();
  currentGame = undefined;
};

const scheduleEmulatorFrame = () => {
  let phase879Frame = 0;
  const update = () => {
    if (!currentEmulator) return;

    phase879Frame += 1;
    try {
      if (phase879Frame <= 8 || phase879Frame % 60 === 0) {
        debugLog(
          "PHASE8_79_FRAME",
          `stage=before-update frame=${phase879Frame}`,
          `game=${currentGame?.name ?? "<unknown>"}`
        );
      }
      currentEmulator.update();
      if (phase879Frame <= 8 || phase879Frame % 60 === 0) {
        debugLog(
          "PHASE8_79_FRAME",
          `stage=after-update frame=${phase879Frame}`,
          `game=${currentGame?.name ?? "<unknown>"}`
        );
      }
      animationFrame = requestAnimationFrame(update);
    } catch (error) {
      const err = error instanceof Error ? error : new Error(String(error));
      debugLog(
        "PHASE8_79_UPDATE_TRAP",
        `frame=${phase879Frame}`,
        `game=${currentGame?.name ?? "<unknown>"}`,
        `name=${err.name}`,
        `message=${err.message}`,
        `stack=${err.stack ?? "<no-stack>"}`
      );
      console.error(error);
      window.alert(`The game stopped: ${err.message}`);
      void showLibrary();
    }
  };

  animationFrame = requestAnimationFrame(update);
};

const launchGame = async (id: string) => {
  debugLog("GAME", `launch requested id=${id}`);
  const game = await library.get(id);
  if (!game) {
    window.alert("This game is no longer in the library.");
    await renderLibrary();
    return;
  }

  await stopCurrentGame();
  currentGame = game;
  setDebugGameScope(game.name);
  debugLog("PHASE8_80_GAME_TEST_BEGIN", `game=${game.name}`, `file=${game.fileName}`, `id=${game.id}`);

  element("library-view").hidden = true;
  element("player-view").hidden = false;
  debugLog("NAV", `view=player game=${game.name}`);
  element("player-title").textContent = game.name;
  element("player-file-name").textContent = game.fileName;
  element("loading-game").hidden = false;
  applyGameDisplaySettings(game, true);

  const canvas = element<HTMLCanvasElement>("canvas");
  const context = canvas.getContext("2d");
  context?.clearRect(0, 0, canvas.width, canvas.height);

  try {
    currentEmulator = new WieWeb(game.fileName, new Uint8Array(game.archive), canvas);
    const pcmSlider = element<HTMLInputElement>("volume-pcm");
    currentEmulator.set_pcm_volume(Number(pcmSlider.value) / 100);

    game.lastPlayedAt = Date.now();
    await library.put(game);
    debugLog("GAME", `launch successful game=${game.name}`, `file=${game.fileName}`);
    scheduleEmulatorFrame();
    void requestWakeLockForPlayer();
  } catch (error) {
    currentEmulator = undefined;
    currentGame = undefined;
    const message = error instanceof Error ? error.message : String(error);
    debugLog("GAME", `launch failed game=${game.name}`, error);
    window.alert(`Could not start ${game.name}: ${message}`);
    await showLibrary();
  } finally {
    element("loading-game").hidden = true;
  }
};

const showLibrary = async () => {
  await stopCurrentGame();
  unlockOrientation();
  element("settings-panel").classList.remove("visible");
  element("player-view").hidden = true;
  element("library-view").hidden = false;
  debugLog("NAV", "view=library");
  setDebugGameScope(undefined);
  await renderLibrary();
};

const openGameActions = (game: GameRecord) => {
  activeActionGameId = game.id;
  element("game-actions-title").textContent = game.name;
  element("game-actions-subtitle").textContent = game.fileName;
  element("game-actions-overlay").hidden = false;
};

const closeGameActions = () => {
  activeActionGameId = undefined;
  element("game-actions-overlay").hidden = true;
};

const initGameActions = () => {
  element("close-game-actions").addEventListener("click", closeGameActions);
  element("game-actions-overlay").addEventListener("click", (event) => {
    if (event.target === event.currentTarget) closeGameActions();
  });

  element("action-play").addEventListener("click", () => {
    const id = activeActionGameId;
    closeGameActions();
    if (id) void launchGame(id);
  });

  element("action-display").addEventListener("click", async () => {
    const id = activeActionGameId;
    if (!id) return;
    closeGameActions();
    await launchGame(id);
    element("settings-panel").classList.add("visible");
    element("settings-panel").setAttribute("aria-hidden", "false");
  });

  element("action-controls").addEventListener("click", async () => {
    const id = activeActionGameId;
    if (!id) return;
    closeGameActions();
    await launchGame(id);
    await openControlEditor();
  });

  element("action-saves").addEventListener("click", async () => {
    const id = activeActionGameId;
    if (!id) return;
    closeGameActions();
    await openSaveManager(id);
  });



  element("action-cover").addEventListener("click", () => {
    if (!activeActionGameId) return;
    pendingCoverGameId = activeActionGameId;
    element<HTMLInputElement>("cover-import-input").click();
  });


  element("action-favorite").addEventListener("click", async () => {
    const id = activeActionGameId;
    if (!id) return;
    const game = await library.get(id);
    if (!game) return;
    game.favorite = !game.favorite;
    await library.put(game);
    closeGameActions();
    successFeedback(game.favorite ? t("toast.favoriteAdded") : t("toast.favoriteRemoved"));
    await renderLibrary();
  });

  element("action-edit").addEventListener("click", async () => {
    const id = activeActionGameId;
    if (!id) return;
    closeGameActions();
    await openGameEditor(id);
  });

  element("action-export-game").addEventListener("click", async () => {
    const id = activeActionGameId;
    if (!id) return;
    const game = await library.get(id);
    if (!game) return;
    closeGameActions();
    try {
      debugLog("LIBRARY", `export entry requested game=${game.name}`);
      await exportGameEntry(game);
      successFeedback("Game entry exported.");
    } catch (error) {
      console.error("Game entry export failed", error);
      errorFeedback("Could not export this game entry.");
    }
  });

  element("action-delete").addEventListener("click", async () => {
    const id = activeActionGameId;
    if (!id) return;
    const game = await library.get(id);
    if (!game) return;

    closeGameActions();
    const confirmed = await requestConfirmation({
      title: "Delete Game?",
      message: `Remove “${game.name}” from My Games?\n\nThe imported game package and custom cover will be deleted from the library. Existing WIE in-game save data and WIPI Player backups are kept unless you erase them separately.`,
      confirmLabel: "Delete Game",
      destructive: true,
    });
    if (!confirmed) return;

    try {
      debugLog("LIBRARY", `delete requested game=${game.name}`, `id=${id}`);
      await library.delete(id);
      invalidateCoverUrl(id);
      const stillExists = await library.get(id);
      if (stillExists) throw new Error("Library record still exists after delete transaction");
      await renderLibrary();
      successFeedback(`${game.name} was removed from My Games.`);
      debugLog("LIBRARY", `delete verified game=${game.name}`, `id=${id}`);
    } catch (error) {
      console.error("Game library delete failed", error);
      errorFeedback("Could not delete this game.");
    }
  });
};

const initFileImport = () => {
  const importInput = element<HTMLInputElement>("game-import-input");
  const coverInput = element<HTMLInputElement>("cover-import-input");
  const entryInput = element<HTMLInputElement>("game-entry-import-input");

  const requestGameImport = () => importInput.click();
  element("import-game").addEventListener("click", requestGameImport);
  element("empty-import-game").addEventListener("click", requestGameImport);

  const importPortableEntry = async (file: File) => {
    const game = await parseGameEntry(file);
    const existing = await library.get(game.id);
    if (existing) {
      const confirmed = await requestConfirmation({
        title: "Replace Existing Game Entry?",
        message: `“${existing.name}” is already in My Games. Replace its package, cover, and WIPI Player settings with this imported entry?\n\nNormal in-game save data is not changed.`,
        confirmLabel: "Replace Entry",
        destructive: true,
      });
      if (!confirmed) return;
    }
    if (existing) game.saveSources = existing.saveSources;
    invalidateCoverUrl(game.id);
    await library.put(game);
    debugLog("LIBRARY", `portable entry imported game=${game.name}`, `id=${game.id}`);
    await renderLibrary();
    successFeedback(existing ? "Game entry replaced." : "Game entry imported.");
  };

  importInput.addEventListener("change", async () => {
    const file = importInput.files?.[0];
    importInput.value = "";
    if (!file) return;

    try {
      const lower = file.name.toLowerCase();
      if (lower.endsWith(".wipigame.json") || lower.endsWith(".json")) {
        await importPortableEntry(file);
      } else {
        await importGame(file);
      }
    } catch (error) {
      console.error(error);
      errorFeedback(`Import failed: ${String(error)}`);
    }
  });

  coverInput.addEventListener("change", async () => {
    const file = coverInput.files?.[0];
    coverInput.value = "";
    const gameId = pendingCoverGameId;
    pendingCoverGameId = undefined;

    if (!file || !gameId) return;

    const game = await library.get(gameId);
    if (!game) return;

    invalidateCoverUrl(game.id);
    game.cover = file;
    await library.put(game);
    closeGameActions();
    await renderLibrary();
  });

  const legacyImportEntryButton = document.getElementById("import-game-entry");
  legacyImportEntryButton?.addEventListener("click", () => entryInput.click());
  entryInput.addEventListener("change", async () => {
    const file = entryInput.files?.[0];
    entryInput.value = "";
    if (!file) return;
    try {
      await importPortableEntry(file);
    } catch (error) {
      console.error("Game entry import failed", error);
      errorFeedback("Could not import this WIPI Player game entry.");
    }
  });
};

const updateLibrarySortUi = () => {
  const sort = document.getElementById("library-sort-home") as HTMLSelectElement | null;
  const directionButton = document.getElementById("library-sort-direction") as HTMLButtonElement | null;
  if (sort) sort.value = appSettings.librarySort;
  if (directionButton) {
    const asc = appSettings.librarySortDirection === "asc";
    directionButton.textContent = asc ? "↑" : "↓";
    const meaning = appSettings.librarySort === "name"
      ? (asc ? t("sort.aToZ") : t("sort.zToA"))
      : appSettings.librarySort === "recent"
        ? (asc ? t("sort.oldest") : t("sort.newest"))
        : (asc ? t("sort.nonFavorites") : t("sort.favoritesFirst"));
    directionButton.ariaLabel = `Reverse sort order. Current: ${meaning}`;
    directionButton.title = meaning;
  }
};

const initLibrarySortControl = () => {
  const sort = document.getElementById("library-sort-home") as HTMLSelectElement | null;
  const directionButton = document.getElementById("library-sort-direction") as HTMLButtonElement | null;
  if (!sort || !directionButton) return;
  updateLibrarySortUi();
  debugLog("LIBRARY", `sort control initialized mode=${appSettings.librarySort} direction=${appSettings.librarySortDirection}`);

  sort.addEventListener("change", async () => {
    appSettings.librarySort = sort.value as AppSettings["librarySort"];
    // Use the conventional default direction when changing sort modes.
    appSettings.librarySortDirection = appSettings.librarySort === "name" ? "asc" : "desc";
    saveAppSettings(appSettings);
    updateLibrarySortUi();
    debugLog("LIBRARY", `home sort=${appSettings.librarySort} direction=${appSettings.librarySortDirection}`);
    await renderLibrary();
  });

  let lastDirectionActivation = 0;
  const reverseSortDirection = async (event?: Event) => {
    event?.preventDefault();
    event?.stopPropagation();
    const now = performance.now();
    if (now - lastDirectionActivation < 300) return;
    lastDirectionActivation = now;
    appSettings.librarySortDirection = appSettings.librarySortDirection === "asc" ? "desc" : "asc";
    saveAppSettings(appSettings);
    // Update the arrow immediately before any IndexedDB/library work.
    updateLibrarySortUi();
    debugLog("LIBRARY", `sort direction=${appSettings.librarySortDirection}`);
    await renderLibrary();
    updateLibrarySortUi();
  };

  directionButton.addEventListener("pointerup", (event) => { void reverseSortDirection(event); });
  directionButton.addEventListener("click", (event) => {
    // Keyboard/accessibility fallback. Pointer taps are handled above and de-duplicated.
    void reverseSortDirection(event);
  });
};

const initInput = () => {
  for (const button of document.querySelectorAll<HTMLButtonElement>("button[data-key]")) {
    const release = (event: Event) => {
      event.preventDefault();
      if (controlEditing) return;
      const key = button.dataset.key;
      button.classList.remove("is-pressed");
      if (key && currentEmulator) currentEmulator.key_up(key);
    };

    button.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      if (controlEditing) return;
      const key = button.dataset.key;
      button.classList.add("is-pressed");
      try {
        button.setPointerCapture((event as PointerEvent).pointerId);
      } catch {
        // setPointerCapture is optional in some embedded WebKit versions.
      }
      if (key && currentEmulator) currentEmulator.key_down(key);
    });
    button.addEventListener("pointerup", release);
    button.addEventListener("pointercancel", release);
    button.addEventListener("contextmenu", (event) => event.preventDefault());
  }

  document.addEventListener("keydown", (event) => {
    const key = keyMap[event.code];
    if (controlEditing || !key || !currentEmulator) return;
    event.preventDefault();
    currentEmulator.key_down(key);
  });

  document.addEventListener("keyup", (event) => {
    const key = keyMap[event.code];
    if (controlEditing || !key || !currentEmulator) return;
    event.preventDefault();
    currentEmulator.key_up(key);
  });
};


const initControlEditor = () => {
  const direction = document.querySelector<HTMLElement>(".direction-pad");
  const number = document.querySelector<HTMLElement>(".number-pad");
  if (!direction || !number) return;

  const beginDrag = (padName: ControlPadName, padElement: HTMLElement, event: PointerEvent) => {
    if (!controlEditing || !currentGame) return;
    event.preventDefault();
    setSelectedControlPad(padName);

    try {
      padElement.setPointerCapture(event.pointerId);
    } catch {
      // Optional in embedded WebKit.
    }

    const move = (moveEvent: PointerEvent) => {
      if (!controlEditing || !currentGame) return;
      const content = document.querySelector<HTMLElement>("#player-view .player-content");
      if (!content) return;
      const rect = content.getBoundingClientRect();
      if (!rect.width || !rect.height) return;

      const x = Math.min(96, Math.max(4, ((moveEvent.clientX - rect.left) / rect.width) * 100));
      const y = Math.min(94, Math.max(6, ((moveEvent.clientY - rect.top) / rect.height) * 100));
      const layout = currentControlLayout();
      if (!layout) return;
      layout[padName].x = x;
      layout[padName].y = y;
      applyControlSettings(currentGame);
    };

    const finish = () => {
      padElement.removeEventListener("pointermove", move);
      padElement.removeEventListener("pointerup", finish);
      padElement.removeEventListener("pointercancel", finish);
      if (currentGame) {
        debugLog("CONTROLS", `drag ${padName}`, currentGame.settings.orientation, currentControlLayout()?.[padName]);
        void saveCurrentGameSettings();
      }
    };

    padElement.addEventListener("pointermove", move);
    padElement.addEventListener("pointerup", finish, { once: true });
    padElement.addEventListener("pointercancel", finish, { once: true });
  };

  direction.addEventListener("pointerdown", (event) => beginDrag("direction", direction, event));
  number.addEventListener("pointerdown", (event) => beginDrag("number", number, event));

  element("control-pad-direction").addEventListener("click", () => setSelectedControlPad("direction"));
  element("control-pad-number").addEventListener("click", () => setSelectedControlPad("number"));
  element("control-editor-done").addEventListener("click", () => void closeControlEditor());

  element("control-preset-classic").addEventListener("click", () => void applyControlStarter("classic"));
  element("control-preset-spacious").addEventListener("click", () => void applyControlStarter("spacious"));
  element("control-preset-compact").addEventListener("click", () => void applyControlStarter("compact"));
  element("control-reset").addEventListener("click", () => void resetControlsForOrientation());

  const size = element<HTMLInputElement>("control-size");
  const gap = element<HTMLInputElement>("control-gap");
  const opacity = element<HTMLInputElement>("control-opacity");

  size.addEventListener("input", () => {
    if (!currentGame) return;
    ensureCustomControls();
    const layout = currentControlLayout();
    if (!layout) return;
    layout[selectedControlPad].scale = Number(size.value) / 100;
    applyControlSettings(currentGame);
  });
  size.addEventListener("change", () => void saveCurrentGameSettings());

  gap.addEventListener("input", () => {
    if (!currentGame) return;
    ensureCustomControls();
    const layout = currentControlLayout();
    if (!layout) return;
    layout[selectedControlPad].gap = Number(gap.value);
    applyControlSettings(currentGame);
  });
  gap.addEventListener("change", () => void saveCurrentGameSettings());

  opacity.addEventListener("input", () => {
    if (!currentGame) return;
    ensureCustomControls();
    const layout = currentControlLayout();
    if (!layout) return;
    layout.opacity = Number(opacity.value) / 100;
    applyControlSettings(currentGame);
  });
  opacity.addEventListener("change", () => void saveCurrentGameSettings());

  const visibility = element("control-key-visibility");
  for (const key of CONTROL_KEYS) {
    const label = document.createElement("label");
    label.className = "control-key-toggle";
    const input = document.createElement("input");
    input.type = "checkbox";
    input.dataset.controlKey = key;
    input.checked = true;
    input.addEventListener("change", () => {
      if (!currentGame) return;
      ensureCustomControls();
      const layout = currentControlLayout();
      if (!layout) return;
      const hidden = new Set(layout.hiddenKeys);
      if (input.checked) hidden.delete(key);
      else hidden.add(key);
      layout.hiddenKeys = Array.from(hidden);
      applyControlSettings(currentGame);
      debugLog("CONTROLS", `key ${key} visible=${input.checked}`, currentGame.settings.orientation);
      void saveCurrentGameSettings();
    });
    const text = document.createElement("span");
    text.textContent = key;
    label.append(input, text);
    visibility.append(label);
  }
};


const formatBackupDate = (timestamp: number): string =>
  new Intl.DateTimeFormat(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(new Date(timestamp));

const updateSaveSourceStatus = (game: GameRecord) => {
  const databases = game.saveSources.databases.length;
  const filesystem = game.saveSources.filesystemAids.length;
  const status = element("save-source-status");
  if (!hasSaveSources(game.saveSources)) {
    status.textContent = "No WIE save storage has been detected yet. Launch the game and reach/use its normal in-game save feature first.";
  } else {
    const parts = [];
    if (databases) parts.push(`${databases} record database${databases === 1 ? "" : "s"}`);
    if (filesystem) parts.push(`${filesystem} filesystem namespace${filesystem === 1 ? "" : "s"}`);
    status.textContent = `Detected: ${parts.join(" + ")}.`;
  }
};

const renderSaveBackups = async (game: GameRecord) => {
  updateSaveSourceStatus(game);
  const list = element("save-backup-list");
  const empty = element("save-backup-empty");
  const backups = await listSaveBackups(game.id);
  list.replaceChildren();
  empty.hidden = backups.length !== 0;

  for (const backup of backups) {
    const row = document.createElement("article");
    row.className = "save-backup-row";

    const meta = document.createElement("div");
    meta.className = "save-backup-meta";
    const title = document.createElement("strong");
    title.textContent = formatBackupDate(backup.createdAt);
    const detail = document.createElement("span");
    const entryCount = backup.databases.reduce(
      (count, db) => count + db.stores.reduce((subtotal, store) => subtotal + store.entries.length, 0),
      0
    );
    detail.textContent = `${entryCount} saved record${entryCount === 1 ? "" : "s"}`;
    meta.append(title, detail);

    const actions = document.createElement("div");
    actions.className = "save-backup-actions";

    const restore = document.createElement("button");
    restore.type = "button";
    restore.textContent = t("saves.restore");
    restore.addEventListener("click", () => void restoreBackupForGame(game, backup));

    const exportButton = document.createElement("button");
    exportButton.type = "button";
    exportButton.textContent = t("saves.export");
    exportButton.addEventListener("click", () => {
      void exportSaveBackup(backup).catch((error) => {
        console.error("Save export failed", error);
        window.alert(`Could not export this save backup: ${String(error)}`);
      });
    });

    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "danger-button";
    remove.textContent = t("saves.delete");
    remove.addEventListener("click", async () => {
      const confirmed = await requestConfirmation({
        title: "Delete Backup?",
        message: `Delete the backup from ${formatBackupDate(backup.createdAt)}?\n\nThis does not erase the game's current in-game save.`,
        confirmLabel: "Delete Backup",
        destructive: true,
      });
      if (!confirmed) return;

      try {
        debugLog("SAVE", `backup delete requested game=${game.name}`, `backup=${backup.id}`);
        await deleteSaveBackup(backup.id);
        const remaining = await listSaveBackups(game.id);
        if (remaining.some((item) => item.id === backup.id)) {
          throw new Error("Backup still exists after delete transaction");
        }
        debugLog("SAVE", `backup delete verified game=${game.name}`, `backup=${backup.id}`);
        await renderSaveBackups(game);
        successFeedback("Backup deleted.");
      } catch (error) {
        console.error("Save backup delete failed", error);
        errorFeedback("Could not delete the backup.");
      }
    });

    actions.append(restore, exportButton, remove);
    row.append(meta, actions);
    list.append(row);
  }
};

const openSaveManager = async (gameId: string) => {
  const game = await library.get(gameId);
  if (!game) return;
  saveManagerGameId = gameId;
  element("save-manager-title").textContent = appSettings.language === "ko" ? `${game.name} — 세이브` : `${game.name} — Saves`;
  element("save-manager-overlay").hidden = false;
  await renderSaveBackups(game);
  debugLog("SAVE", `manager opened game=${game.name}`, game.saveSources);
};

const closeSaveManager = () => {
  saveManagerGameId = undefined;
  element("save-manager-overlay").hidden = true;
};

const createBackupForGame = async (game: GameRecord) => {
  if (!hasSaveSources(game.saveSources)) {
    window.alert("No save storage has been detected for this game yet. Launch it and use its normal in-game save function first, then try again.");
    return;
  }
  const backup = await createSaveBackup(game.id, game.name, game.saveSources);
  debugLog("SAVE", `backup created game=${game.name}`, `backup=${backup.id}`);
  await renderSaveBackups(game);
  successFeedback("Backup created.");
};

const restoreBackupForGame = async (game: GameRecord, backup: SaveBackup) => {
  const confirmed = await requestConfirmation({
    title: "Restore Backup?",
    message: `Restore the backup from ${formatBackupDate(backup.createdAt)}?\n\nThis replaces the game's current in-game save data.`,
    confirmLabel: "Restore Backup",
  });
  if (!confirmed) return;
  const wasRunning = currentGame?.id === game.id;
  if (wasRunning) stopCurrentGame();
  await restoreSaveBackup(backup);
  debugLog("SAVE", `backup restored game=${game.name}`, `backup=${backup.id}`);
  successFeedback("Save backup restored.");
  if (wasRunning) {
    closeSaveManager();
    await launchGame(game.id);
  } else {
    await renderSaveBackups(game);
  }
};

const eraseSavesForGame = async (game: GameRecord) => {
  if (!hasSaveSources(game.saveSources)) {
    window.alert("No save storage has been detected for this game.");
    return;
  }
  const confirmed = await requestConfirmation({
    title: "Erase Current Save?",
    message: `Erase ${game.name}'s current in-game save data?\n\nWIPI Player backups are kept, so you can restore one later. This action cannot otherwise be undone.`,
    confirmLabel: "Erase Save",
    destructive: true,
  });
  if (!confirmed) return;
  const wasRunning = currentGame?.id === game.id;
  if (wasRunning) stopCurrentGame();
  await eraseGameSaveData(game.saveSources);
  debugLog("SAVE", `current save erased game=${game.name}`);
  successFeedback("Current in-game save was erased. Backups were kept.");
  if (wasRunning) {
    closeSaveManager();
    await launchGame(game.id);
  } else {
    await renderSaveBackups(game);
  }
};

const initSaveManagement = () => {
  window.addEventListener("wie-save-storage-access", (rawEvent) => {
    if (!currentGame) return;
    const event = rawEvent as CustomEvent<{ dbName?: string; key?: IDBValidKey }>;
    const dbName = event.detail?.dbName;
    if (!dbName) return;

    let changed = false;
    if (dbName === "wie_filesystem") {
      const key = event.detail.key;
      if (Array.isArray(key) && typeof key[0] === "string" && !currentGame.saveSources.filesystemAids.includes(key[0])) {
        currentGame.saveSources.filesystemAids.push(key[0]);
        changed = true;
      }
    } else if (dbName.startsWith("wie_") && !currentGame.saveSources.databases.includes(dbName)) {
      currentGame.saveSources.databases.push(dbName);
      changed = true;
    }

    if (changed) {
      debugLog("SAVE", `storage associated game=${currentGame.name}`, currentGame.saveSources);
      void library.put(currentGame);
    }
  });

  element("save-manager-close").addEventListener("click", closeSaveManager);
  element("save-manager-overlay").addEventListener("click", (event) => {
    if (event.target === event.currentTarget) closeSaveManager();
  });

  element("save-create-backup").addEventListener("click", async () => {
    const game = saveManagerGameId ? await library.get(saveManagerGameId) : undefined;
    if (!game) return;
    try {
      await createBackupForGame(game);
    } catch (error) {
      console.error("Save backup failed", error);
      window.alert(`Could not create a save backup: ${String(error)}`);
    }
  });

  element("save-import-backup").addEventListener("click", () => element<HTMLInputElement>("save-import-input").click());
  element<HTMLInputElement>("save-import-input").addEventListener("change", async (event) => {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file || !saveManagerGameId) return;
    const game = await library.get(saveManagerGameId);
    if (!game) return;
    try {
      const parsed = await parseImportedSaveBackup(file);
      if (parsed.gameId !== game.id) {
        const confirmed = await requestConfirmation({
          title: "Backup Is for Another Game",
          message: `This backup was created for “${parsed.gameName}”, not “${game.name}”. Import it anyway?`,
          confirmLabel: "Import Anyway",
          destructive: true,
        });
        if (!confirmed) return;
      }
      const imported = await storeImportedSaveBackup(parsed, game.id, game.name);
      debugLog("SAVE", `backup imported game=${game.name}`, `backup=${imported.id}`);
      await renderSaveBackups(game);
      successFeedback("Backup imported.");
    } catch (error) {
      console.error("Save import failed", error);
      window.alert(`Could not import this save backup: ${String(error)}`);
    }
  });

  element("save-erase-current").addEventListener("click", async () => {
    const game = saveManagerGameId ? await library.get(saveManagerGameId) : undefined;
    if (!game) return;
    try {
      await eraseSavesForGame(game);
    } catch (error) {
      console.error("Save erase failed", error);
      window.alert(`Could not erase this game's save data: ${String(error)}`);
    }
  });
};


const openGameEditor = async (id: string) => {
  const game = await library.get(id);
  if (!game) return;
  pendingEditGameId = id;
  element<HTMLInputElement>("edit-game-name").value = game.name;
  element("edit-game-file").textContent = game.fileName;
  element("game-editor-overlay").hidden = false;
  debugLog("LIBRARY", `editor opened game=${game.name}`);
};

const closeGameEditor = () => {
  pendingEditGameId = undefined;
  element("game-editor-overlay").hidden = true;
};

const initGameEditor = () => {
  element("game-editor-close").addEventListener("click", closeGameEditor);
  element("game-editor-cancel").addEventListener("click", closeGameEditor);
  element("game-editor-overlay").addEventListener("click", (event) => {
    if (event.target === event.currentTarget) closeGameEditor();
  });
  element("game-editor-save").addEventListener("click", async () => {
    if (!pendingEditGameId) return;
    const game = await library.get(pendingEditGameId);
    if (!game) return;
    const name = element<HTMLInputElement>("edit-game-name").value.trim();
    if (!name) {
      errorFeedback("Game name cannot be empty.");
      return;
    }
    game.name = name;
    await library.put(game);
    closeGameEditor();
    await renderLibrary();
    successFeedback("Game details saved.");
  });
  element("game-editor-cover").addEventListener("click", () => {
    if (!pendingEditGameId) return;
    pendingCoverGameId = pendingEditGameId;
    element<HTMLInputElement>("cover-import-input").click();
  });
};

const openHomeSettings = () => {
  const homeSort = document.getElementById("home-sort-mode") as HTMLSelectElement | null;
  if (homeSort) homeSort.value = appSettings.librarySort;
  element<HTMLSelectElement>("home-default-orientation").value = appSettings.defaultOrientation;
  element<HTMLSelectElement>("home-default-display").value = appSettings.defaultDisplayMode;
  element<HTMLInputElement>("home-keep-awake").checked = appSettings.keepScreenAwake;
  const language = document.getElementById("home-language") as HTMLSelectElement | null;
  if (language) language.value = appSettings.language;
  element("home-settings-overlay").hidden = false;
  debugLog("SETTINGS", "home settings opened");
};

const closeHomeSettings = () => {
  element("home-settings-overlay").hidden = true;
};

const initHomeSettings = () => {
  element("library-settings").addEventListener("click", openHomeSettings);
  element("home-settings-close").addEventListener("click", closeHomeSettings);
  element("home-settings-done").addEventListener("click", closeHomeSettings);
  element("home-settings-overlay").addEventListener("click", (event) => {
    if (event.target === event.currentTarget) closeHomeSettings();
  });

  const settingsSort = document.getElementById("home-sort-mode") as HTMLSelectElement | null;
  settingsSort?.addEventListener("change", async (event) => {
    appSettings.librarySort = (event.currentTarget as HTMLSelectElement).value as AppSettings["librarySort"];
    saveAppSettings(appSettings);
    const homeSort = document.getElementById("library-sort-home") as HTMLSelectElement | null;
    if (homeSort) homeSort.value = appSettings.librarySort;
    appSettings.librarySortDirection = appSettings.librarySort === "name" ? "asc" : "desc";
    saveAppSettings(appSettings);
    updateLibrarySortUi();
    debugLog("SETTINGS", `library sort=${appSettings.librarySort} direction=${appSettings.librarySortDirection}`);
    await renderLibrary();
  });
  element<HTMLSelectElement>("home-default-orientation").addEventListener("change", (event) => {
    appSettings.defaultOrientation = (event.currentTarget as HTMLSelectElement).value === "landscape" ? "landscape" : "portrait";
    saveAppSettings(appSettings);
    debugLog("SETTINGS", `default orientation=${appSettings.defaultOrientation}`);
  });
  element<HTMLSelectElement>("home-default-display").addEventListener("change", (event) => {
    appSettings.defaultDisplayMode = (event.currentTarget as HTMLSelectElement).value as AppSettings["defaultDisplayMode"];
    saveAppSettings(appSettings);
    debugLog("SETTINGS", `default display=${appSettings.defaultDisplayMode}`);
  });
  element<HTMLInputElement>("home-keep-awake").addEventListener("change", (event) => {
    appSettings.keepScreenAwake = (event.currentTarget as HTMLInputElement).checked;
    saveAppSettings(appSettings);
    debugLog("SETTINGS", `keep screen awake=${appSettings.keepScreenAwake}`);
    if (appSettings.keepScreenAwake) void requestWakeLockForPlayer();
    else void releaseWakeLock();
  });
  document.getElementById("home-language")?.addEventListener("change", async (event) => {
    appSettings.language = (event.currentTarget as HTMLSelectElement).value === "ko" ? "ko" : "en";
    saveAppSettings(appSettings);
    setLanguage(appSettings.language);
    applyTranslations();
    updateLibrarySortUi();
    await renderLibrary();
    debugLog("SETTINGS", `language=${appSettings.language}`);
  });
};

const openGlobalDiagnostics = () => {
  element<HTMLTextAreaElement>("debug-log-text").value = getDebugLogText();
  element("debug-session-status").textContent = `Current session: ${getDebugSessionId() || "starting"} · ${getDebugLogText().split("\n").filter(Boolean).length} log lines`;
  element("debug-log-overlay").hidden = false;
  debugLog("DIAGNOSTICS", "viewer opened");
};

const refreshGlobalDiagnostics = () => {
  element<HTMLTextAreaElement>("debug-log-text").value = getDebugLogText();
  element("debug-session-status").textContent = `Current session: ${getDebugSessionId() || "starting"} · ${getDebugLogText().split("\n").filter(Boolean).length} log lines`;
};

const initGlobalDiagnostics = () => {
  element("library-diagnostics").addEventListener("click", openGlobalDiagnostics);
  element("debug-refresh-log").addEventListener("click", refreshGlobalDiagnostics);
  element("debug-export-log-global").addEventListener("click", () => {
    debugLog("DIAGNOSTICS", "export requested from global viewer");
    void exportDebugLog().catch((error) => console.error("Failed to export diagnostic log", error));
  });
  element("debug-clear-log-global").addEventListener("click", async () => {
    const confirmed = await requestConfirmation({
      title: "Clear Diagnostic Log?",
      message: "Clear all persisted WIPI Player diagnostic sessions?",
      confirmLabel: "Clear Log",
      destructive: true,
    });
    if (!confirmed) return;
    clearDebugLog();
    refreshGlobalDiagnostics();
    successFeedback("Diagnostic log cleared.");
  });
};

const initPlayerChrome = () => {
  element("back-to-library").addEventListener("click", () => void showLibrary());

  const toggle = element("settings-toggle");
  const panel = element("settings-panel");
  const orientationSelect = element<HTMLSelectElement>("game-orientation");
  const displayModeSelect = element<HTMLSelectElement>("game-display-mode");

  const togglePanel = () => {
    panel.classList.toggle("visible");
    panel.setAttribute("aria-hidden", panel.classList.contains("visible") ? "false" : "true");
  };

  toggle.addEventListener("click", (event) => {
    event.stopPropagation();
    togglePanel();
  });

  element("customize-controls").addEventListener("click", () => void openControlEditor());
  element("manage-saves").addEventListener("click", () => {
    if (currentGame) void openSaveManager(currentGame.id);
  });

  element("rotate-screen").addEventListener("click", async (event) => {
    event.stopPropagation();
    if (!currentGame) return;

    currentGame.settings.orientation =
      currentGame.settings.orientation === "portrait" ? "landscape" : "portrait";
    debugLog("DISPLAY", `rotate button -> ${currentGame.settings.orientation}`);
    applyGameDisplaySettings(currentGame, true);
    await saveCurrentGameSettings();
  });

  orientationSelect.addEventListener("change", async () => {
    if (!currentGame) return;

    currentGame.settings.orientation =
      orientationSelect.value === "landscape" ? "landscape" : "portrait";
    debugLog("DISPLAY", `orientation select -> ${currentGame.settings.orientation}`);
    applyGameDisplaySettings(currentGame, true);
    await saveCurrentGameSettings();
  });

  displayModeSelect.addEventListener("change", async () => {
    if (!currentGame) return;

    const mode = displayModeSelect.value;
    if (mode === "native" || mode === "compact" || mode === "fit" || mode === "large" || mode === "max") {
      currentGame.settings.displayMode = mode;
      debugLog("DISPLAY", `display mode -> ${mode}`);
      applyGameDisplaySettings(currentGame);
      await saveCurrentGameSettings();
    }
  });

  element("debug-view-log").addEventListener("click", openGlobalDiagnostics);

  element("debug-export-log").addEventListener("click", () => {
    void exportDebugLog().catch((error) => {
      console.error("Failed to export diagnostic log", error);
      window.alert(`Could not export the diagnostic log: ${String(error)}`);
    });
  });

  element("debug-clear-log").addEventListener("click", async () => {
    const confirmed = await requestConfirmation({
      title: "Clear Diagnostic Log?",
      message: "Clear the temporary WIPI Player diagnostic log?",
      confirmLabel: "Clear Log",
      destructive: true,
    });
    if (!confirmed) return;
    clearDebugLog();
    element<HTMLTextAreaElement>("debug-log-text").value = getDebugLogText();
    successFeedback("Diagnostic log cleared.");
  });

  element("debug-close-log").addEventListener("click", () => {
    element("debug-log-overlay").hidden = true;
  });
  element("debug-log-overlay").addEventListener("click", (event) => {
    if (event.target === event.currentTarget) element("debug-log-overlay").hidden = true;
  });

  const viewportChanged = () => schedulePlayerLayoutRefresh("viewport-change");
  window.addEventListener("resize", viewportChanged);
  window.addEventListener("orientationchange", viewportChanged);
  screen.orientation?.addEventListener?.("change", viewportChanged);
  window.visualViewport?.addEventListener("resize", viewportChanged);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible" && currentGame) void requestWakeLockForPlayer();
  });

  document.addEventListener("click", (event) => {
    if (!toggle.contains(event.target as Node) && !panel.contains(event.target as Node)) {
      panel.classList.remove("visible");
      panel.setAttribute("aria-hidden", "true");
    }
  });
};

const inspectSaveStateCapability = () => {
  const emulator = currentEmulator as unknown as Record<string, unknown> | undefined;
  const methods = emulator ? Object.getOwnPropertyNames(Object.getPrototypeOf(emulator)).sort() : [];
  const hasSerializer = Boolean(emulator && (typeof emulator["save_state"] === "function" || typeof emulator["serialize"] === "function"));
  const report = {
    game: currentGame?.name ?? null,
    emulatorLoaded: Boolean(emulator),
    exportedMethods: methods,
    fullRuntimeSerializerAvailable: hasSerializer,
    conclusion: hasSerializer
      ? "A runtime serializer export exists and can be investigated further."
      : "Current WIE WebAssembly API exposes no full-runtime save-state serializer. Native persistent saves remain supported through Save Manager.",
  };
  debugLog("STATE_LAB", "runtime capability inspected", report);
  const output = document.getElementById("state-lab-output");
  if (output) output.textContent = report.conclusion + ` Exported methods: ${methods.join(", ") || "none"}.`;
  return report;
};

const initSaveIoDiagnostics = () => {
  window.addEventListener("wie-save-write-committed", (event) => {
    const detail = (event as CustomEvent).detail as { dbName?: string; storeName?: string; key?: IDBValidKey; bytes?: number; ms?: number };
    debugLog("SAVE_IO", `write committed db=${detail.dbName ?? "?"} store=${detail.storeName ?? "?"} bytes=${detail.bytes ?? 0} ms=${detail.ms ?? 0}`, detail.key);
  });
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState !== "hidden") return;
    const pending = IndexedDBStore.getPendingWriteCount();
    debugLog("SAVE_IO", `app hiding pendingWrites=${pending}`);
    void IndexedDBStore.flushPendingWrites().then(() => debugLog("SAVE_IO", "background flush completed"));
  });
};

const main = async () => {
  debugLog("BOOT", "main() starting");
  initUiFeedback();
  setLanguage(appSettings.language);
  applyTranslations();
  initGlobalDiagnostics();
  initSaveIoDiagnostics();
  library = await GameLibrary.open();
  debugLog("BOOT", "GameLibrary opened");
  initTutorial();
  document.getElementById("state-lab-inspect")?.addEventListener("click", () => {
    inspectSaveStateCapability();
    successFeedback("State capability report added to Logs.");
  });
  restoreVolumeControls();
  initFileImport();
  initGameActions();
  initGameEditor();
  initHomeSettings();
  initLibrarySortControl();
  initInput();
  initControlEditor();
  initSaveManagement();
  initPlayerChrome();
  await renderLibrary();
};

const boot = () => {
  void main().catch((error) => {
    console.error(error);
    window.alert(`WIPI Player failed to start: ${String(error)}`);
  });
};

if (document.readyState !== "loading") {
  boot();
} else {
  document.addEventListener("DOMContentLoaded", boot);
}
