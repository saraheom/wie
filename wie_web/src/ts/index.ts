import { WieWeb } from "@pkg";
import { setMasterVolume } from "./midi";
import { clearDebugLog, debugLog, exportDebugLog, getDebugLogText, installDebugLogging } from "./debug_log";
import {
  defaultGameSettings,
  displayNameForFile,
  GameLibrary,
  GameRecord,
  gameIdForArchive,
} from "./game_library";

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
let animationFrame: number | undefined;
let activeActionGameId: string | undefined;
let pendingCoverGameId: string | undefined;
let renderedCoverUrls: string[] = [];

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
  for (const url of renderedCoverUrls) URL.revokeObjectURL(url);
  renderedCoverUrls = [];
};

const makeCoverElement = (game: GameRecord): HTMLElement => {
  if (game.cover) {
    const image = document.createElement("img");
    const url = URL.createObjectURL(game.cover);
    renderedCoverUrls.push(url);
    image.src = url;
    image.alt = `${game.name} cover`;
    image.className = "game-cover";
    return image;
  }

  const placeholder = document.createElement("span");
  placeholder.className = "game-cover-placeholder";
  const normalized = game.name.trim();
  placeholder.textContent = normalized ? Array.from(normalized)[0].toUpperCase() : "G";
  return placeholder;
};

const renderLibrary = async () => {
  const games = await library.list();
  const grid = element("game-grid");
  const empty = element("library-empty");

  clearRenderedCoverUrls();
  grid.replaceChildren();
  empty.hidden = games.length !== 0;

  for (const game of games) {
    const card = document.createElement("article");
    card.className = "game-card";
    card.dataset.gameId = game.id;

    const coverButton = document.createElement("button");
    coverButton.type = "button";
    coverButton.className = "game-cover-button";
    coverButton.ariaLabel = `Play ${game.name}`;
    coverButton.appendChild(makeCoverElement(game));
    coverButton.addEventListener("click", () => void launchGame(game.id));

    const meta = document.createElement("div");
    meta.className = "game-meta";

    const name = document.createElement("span");
    name.className = "game-name";
    name.textContent = game.name;

    const fileName = document.createElement("span");
    fileName.className = "game-file";
    fileName.textContent = game.fileName;

    const menuButton = document.createElement("button");
    menuButton.type = "button";
    menuButton.className = "game-menu-button";
    menuButton.textContent = "⋯";
    menuButton.ariaLabel = `${game.name} options`;
    menuButton.addEventListener("click", () => openGameActions(game));

    meta.append(name, fileName, menuButton);
    card.append(coverButton, meta);
    grid.appendChild(card);
  }
};

const importGame = async (file: File) => {
  debugLog("LIBRARY", `import start file=${file.name}`, `size=${file.size}`);
  const data = await file.arrayBuffer();
  const id = await gameIdForArchive(data);
  const existing = await library.get(id);

  if (existing) {
    const shouldOpen = window.confirm(`${existing.name} is already in your library. Open it now?`);
    if (shouldOpen) await launchGame(id);
    return;
  }

  const game: GameRecord = {
    id,
    name: displayNameForFile(file.name),
    fileName: file.name,
    archive: data,
    createdAt: Date.now(),
    settings: defaultGameSettings(),
  };

  await library.put(game);
  debugLog("LIBRARY", `import complete game=${game.name}`, `id=${game.id}`);
  await renderLibrary();
};

const stopCurrentGame = () => {
  if (animationFrame !== undefined) {
    cancelAnimationFrame(animationFrame);
    animationFrame = undefined;
  }

  if (currentEmulator) {
    try {
      currentEmulator.free();
    } catch (error) {
      console.warn("Failed to free emulator cleanly", error);
    }
  }

  currentEmulator = undefined;
  currentGame = undefined;
};

const scheduleEmulatorFrame = () => {
  const update = () => {
    if (!currentEmulator) return;

    try {
      currentEmulator.update();
      animationFrame = requestAnimationFrame(update);
    } catch (error) {
      console.error(error);
      const message = error instanceof Error ? error.message : String(error);
      window.alert(`The game stopped: ${message}`);
      stopCurrentGame();
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

  stopCurrentGame();
  currentGame = game;

  element("library-view").hidden = true;
  element("player-view").hidden = false;
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
  stopCurrentGame();
  unlockOrientation();
  element("settings-panel").classList.remove("visible");
  element("player-view").hidden = true;
  element("library-view").hidden = false;
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

  element("action-rename").addEventListener("click", async () => {
    const id = activeActionGameId;
    if (!id) return;
    const game = await library.get(id);
    if (!game) return;

    const nextName = window.prompt("Game name", game.name)?.trim();
    if (!nextName || nextName === game.name) return;

    game.name = nextName;
    await library.put(game);
    closeGameActions();
    await renderLibrary();
  });

  element("action-cover").addEventListener("click", () => {
    if (!activeActionGameId) return;
    pendingCoverGameId = activeActionGameId;
    element<HTMLInputElement>("cover-import-input").click();
  });

  element("action-delete").addEventListener("click", async () => {
    const id = activeActionGameId;
    if (!id) return;
    const game = await library.get(id);
    if (!game) return;

    const confirmed = window.confirm(
      `Delete ${game.name} from the WIPI Player library?\n\nThe imported package and cover will be removed. Existing in-game WIE save data will be kept.`
    );
    if (!confirmed) return;

    await library.delete(id);
    closeGameActions();
    await renderLibrary();
  });
};

const initFileImport = () => {
  const importInput = element<HTMLInputElement>("game-import-input");
  const coverInput = element<HTMLInputElement>("cover-import-input");

  const requestGameImport = () => importInput.click();
  element("import-game").addEventListener("click", requestGameImport);
  element("empty-import-game").addEventListener("click", requestGameImport);

  importInput.addEventListener("change", async () => {
    const file = importInput.files?.[0];
    importInput.value = "";
    if (!file) return;

    try {
      await importGame(file);
    } catch (error) {
      console.error(error);
      window.alert(`Import failed: ${String(error)}`);
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

    game.cover = file;
    await library.put(game);
    closeGameActions();
    await renderLibrary();
  });
};

const initInput = () => {
  for (const button of document.querySelectorAll<HTMLButtonElement>("button[data-key]")) {
    const release = (event: Event) => {
      event.preventDefault();
      const key = button.dataset.key;
      button.classList.remove("is-pressed");
      if (key && currentEmulator) currentEmulator.key_up(key);
    };

    button.addEventListener("pointerdown", (event) => {
      event.preventDefault();
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
    if (!key || !currentEmulator) return;
    event.preventDefault();
    currentEmulator.key_down(key);
  });

  document.addEventListener("keyup", (event) => {
    const key = keyMap[event.code];
    if (!key || !currentEmulator) return;
    event.preventDefault();
    currentEmulator.key_up(key);
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

  element("debug-view-log").addEventListener("click", () => {
    element<HTMLTextAreaElement>("debug-log-text").value = getDebugLogText();
    element("debug-log-overlay").hidden = false;
  });

  element("debug-export-log").addEventListener("click", () => {
    void exportDebugLog().catch((error) => {
      console.error("Failed to export diagnostic log", error);
      window.alert(`Could not export the diagnostic log: ${String(error)}`);
    });
  });

  element("debug-clear-log").addEventListener("click", () => {
    if (!window.confirm("Clear the temporary WIPI Player diagnostic log?")) return;
    clearDebugLog();
    element<HTMLTextAreaElement>("debug-log-text").value = getDebugLogText();
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

  document.addEventListener("click", (event) => {
    if (!toggle.contains(event.target as Node) && !panel.contains(event.target as Node)) {
      panel.classList.remove("visible");
      panel.setAttribute("aria-hidden", "true");
    }
  });
};

const main = async () => {
  library = await GameLibrary.open();
  initTutorial();
  restoreVolumeControls();
  initFileImport();
  initGameActions();
  initInput();
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
