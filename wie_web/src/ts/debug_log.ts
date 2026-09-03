const STORAGE_KEY = "wipi_player_debug_log_v1";
const GAME_LOG_PREFIX = "wipi_player_debug_game_v1:";
const GAME_INDEX_KEY = "wipi_player_debug_game_index_v1";
const LAST_BREADCRUMB_KEY = "wipi_player_debug_last_breadcrumb_v1";
const NATIVE_RING_KEY = "wipi_player_debug_native_ring_v1";
const NATIVE_RING_MAX = 64; // Phase 8.81: force-close-safe native boundary history
const MAX_LINES = 16000; // Phase 8.80: retain multiple compatibility runs in one export
const MAX_CHARS = 4_000_000;
const MAX_GAME_LINES = 8000; // Independent per-game history survives global rolling pressure
const MAX_GAME_CHARS = 2_000_000;

let lines: string[] = [];
let flushTimer: number | undefined;
let installed = false;
let sessionId = "";
let gameScope = "";
const gameLines = new Map<string, string[]>();
let nativeRing: string[] = [];

const formatValue = (value: unknown): string => {
  if (value instanceof Error) {
    return value.stack || `${value.name}: ${value.message}`;
  }
  if (typeof value === "string") return value;
  if (typeof value === "undefined") return "undefined";
  if (typeof value === "function") return `[Function ${value.name || "anonymous"}]`;
  try {
    return JSON.stringify(value);
  } catch {
    try {
      return String(value);
    } catch {
      return "[Unprintable value]";
    }
  }
};

const safeScopeKey = (value: string) => encodeURIComponent(value).slice(0, 180);


const compactNativeBoundary = (category: string, payload: string): string | null => {
  // Phase 8.81: retain only execution boundaries that can identify a native
  // hang/crash. Keep the stored form compact because it is synchronously
  // committed to localStorage before the emulated operation proceeds.
  const marker = `${category} ${payload}`;
  const interesting =
    marker.includes("PHASE8_81_TARGET_WIPIC_") ||
    marker.includes("PHASE8_82_") ||
    marker.includes("PHASE8_83_") ||
    marker.includes("PHASE8_84_") ||
    marker.includes("PHASE8_85_") ||
    marker.includes("PHASE8_64_OZ_JAVA_CALL_ENTRY") ||
    marker.includes("PHASE8_64_OZ_JAVA_CALL_RETURN") ||
    marker.includes("PHASE8_80_GENERIC_VIRTUAL_JAR_") ||
    marker.includes("PHASE8_77_OZ_KPOOL_VIRTUAL_READ_") ||
    marker.includes("PHASE8_71_OZ_METADATA_") ||
    marker.includes("PHASE8_78_OZ_WIE_RUSTJAR_NEGATIVE_HIT") ||
    marker.includes("PHASE8_62_OZ_NETWORK_CONNECT") ||
    marker.includes("PHASE8_79_WEB_UPDATE_STAGE") ||
    marker.includes("PHASE8_79_PRESENT_PROBE") ||
    category === "PHASE8_79_FRAME";
  if (!interesting) return null;

  // Strip the tracing timestamp/prefix while preserving the marker payload.
  const phaseAt = payload.indexOf("[PHASE");
  const compactPayload = phaseAt >= 0 ? payload.slice(phaseAt) : `${category} ${payload}`;
  return `[${new Date().toISOString()}] game=${gameScope || "<none>"} ${compactPayload}`.slice(0, 1200);
};

const persistNativeBoundary = (category: string, payload: string) => {
  const compact = compactNativeBoundary(category, payload);
  if (!compact) return;
  nativeRing.push(compact);
  if (nativeRing.length > NATIVE_RING_MAX) nativeRing.splice(0, nativeRing.length - NATIVE_RING_MAX);
  try {
    // Synchronous on purpose: this must survive a WebView/app force-close that
    // occurs before the normal 400 ms batched diagnostic flush.
    localStorage.setItem(NATIVE_RING_KEY, JSON.stringify(nativeRing));
  } catch {
    // Diagnostics must never affect emulation.
  }
};

const trimArray = (target: string[], maxLines: number, maxChars: number) => {
  if (target.length > maxLines) target.splice(0, target.length - maxLines);
  let chars = target.reduce((sum, line) => sum + line.length + 1, 0);
  while (chars > maxChars && target.length > 1) {
    const removed = target.shift();
    chars -= (removed?.length ?? 0) + 1;
  }
};

const trimLog = () => {
  trimArray(lines, MAX_LINES, MAX_CHARS);
  for (const scoped of gameLines.values()) trimArray(scoped, MAX_GAME_LINES, MAX_GAME_CHARS);
};

const flush = () => {
  flushTimer = undefined;
  try {
    localStorage.setItem(STORAGE_KEY, lines.join("\n"));
    const keys = [...gameLines.keys()];
    localStorage.setItem(GAME_INDEX_KEY, JSON.stringify(keys));
    for (const key of keys) {
      localStorage.setItem(`${GAME_LOG_PREFIX}${safeScopeKey(key)}`, (gameLines.get(key) ?? []).join("\n"));
    }
  } catch {
    // Diagnostic logging must never interfere with emulation.
  }
};

const scheduleFlush = () => {
  if (flushTimer !== undefined) return;
  flushTimer = window.setTimeout(flush, 400);
};

export const debugLog = (category: string, ...values: unknown[]) => {
  const timestamp = new Date().toISOString();
  const payload = values.map(formatValue).join(" ");
  const line = `[${timestamp}] [${category}] ${payload}`.trimEnd();
  lines.push(line);
  if (gameScope) {
    const scoped = gameLines.get(gameScope) ?? [];
    scoped.push(line);
    gameLines.set(gameScope, scoped);
  }
  persistNativeBoundary(category, payload);
  trimLog();

  // Phase 8.80: synchronously persist high-value breadcrumbs. If iOS kills or
  // freezes the webview before the normal batched flush, the next launch can
  // still report the last known emulator boundary.
  if (category.startsWith("PHASE") || category === "GAME" || category === "WINDOW:ERROR" || category === "WINDOW:UNHANDLED_REJECTION") {
    try {
      localStorage.setItem(LAST_BREADCRUMB_KEY, line);
    } catch {
      // Never let diagnostics affect emulation.
    }
  }
  scheduleFlush();
};

export const setDebugGameScope = (name?: string) => {
  gameScope = name?.trim() ?? "";
  if (gameScope && !gameLines.has(gameScope)) gameLines.set(gameScope, []);
  debugLog("PHASE8_80_DIAGNOSTIC_SCOPE", `game=${gameScope || "<none>"}`);
};

export const getDebugLogText = () => {
  const sections = [lines.join("\n")];
  for (const [name, scoped] of gameLines) {
    sections.push(`\n===== PHASE8_80 PER-GAME LOG: ${name} =====\n${scoped.join("\n")}`);
  }
  sections.push(`\n===== PHASE8_81 LIVE NATIVE RING (LAST ${NATIVE_RING_MAX}) =====\n${nativeRing.join("\n")}`);
  return sections.join("\n");
};

export const getDebugSessionId = () => sessionId;

export const clearDebugLog = () => {
  lines = [];
  gameLines.clear();
  gameScope = "";
  try {
    localStorage.removeItem(STORAGE_KEY);
    const storedIndex = JSON.parse(localStorage.getItem(GAME_INDEX_KEY) || "[]") as string[];
    for (const key of storedIndex) localStorage.removeItem(`${GAME_LOG_PREFIX}${safeScopeKey(key)}`);
    localStorage.removeItem(GAME_INDEX_KEY);
    localStorage.removeItem(LAST_BREADCRUMB_KEY);
    localStorage.removeItem(NATIVE_RING_KEY);
    nativeRing = [];
  } catch {
    // Ignore storage failures.
  }
  debugLog("SYSTEM", "Diagnostic log cleared");
};

const exportWithDownload = (file: File) => {
  const url = URL.createObjectURL(file);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = file.name;
  anchor.style.display = "none";
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 2_000);
};

export const exportDebugLog = async () => {
  debugLog("SYSTEM", "Exporting diagnostic log");
  flush();

  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  const file = new File([getDebugLogText()], `WIPI-Player-Debug-${stamp}.txt`, {
    type: "text/plain;charset=utf-8",
  });

  const shareNavigator = navigator as Navigator & {
    canShare?: (data: ShareData) => boolean;
    share?: (data: ShareData) => Promise<void>;
  };

  try {
    const shareData: ShareData = {
      title: "WIPI Player Diagnostic Log",
      text: "Temporary WIPI Player diagnostic log for testing.",
      files: [file],
    };
    if (shareNavigator.share && (!shareNavigator.canShare || shareNavigator.canShare(shareData))) {
      await shareNavigator.share(shareData);
      return;
    }
  } catch (error) {
    // A user cancelling the share sheet is not fatal; fall back to download.
    debugLog("SYSTEM", "Share-sheet export unavailable or cancelled", error);
  }

  exportWithDownload(file);
};

export const installDebugLogging = () => {
  if (installed) return;
  installed = true;

  let recoveredBreadcrumb = "";
  let recoveredNativeRing: string[] = [];
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) lines = stored.split("\n").filter(Boolean);
    const storedIndex = JSON.parse(localStorage.getItem(GAME_INDEX_KEY) || "[]") as string[];
    for (const key of storedIndex) {
      const storedGame = localStorage.getItem(`${GAME_LOG_PREFIX}${safeScopeKey(key)}`);
      if (storedGame) gameLines.set(key, storedGame.split("\n").filter(Boolean));
    }
    recoveredBreadcrumb = localStorage.getItem(LAST_BREADCRUMB_KEY) || "";
    const storedRing = JSON.parse(localStorage.getItem(NATIVE_RING_KEY) || "[]") as unknown;
    if (Array.isArray(storedRing)) {
      recoveredNativeRing = storedRing.filter((entry): entry is string => typeof entry === "string").slice(-NATIVE_RING_MAX);
    }
    // Start a fresh live ring only after preserving the previous run for dump.
    nativeRing = [];
    localStorage.removeItem(NATIVE_RING_KEY);
  } catch {
    lines = [];
    gameLines.clear();
    nativeRing = [];
    recoveredNativeRing = [];
  }

  const consoleObject = console as Console & Record<string, unknown>;
  for (const level of ["log", "info", "warn", "error", "debug"] as const) {
    const original = console[level].bind(console);
    consoleObject[level] = (...args: unknown[]) => {
      debugLog(`CONSOLE:${level.toUpperCase()}`, ...args);
      original(...args);
    };
  }

  window.addEventListener("error", (event) => {
    debugLog(
      "WINDOW:ERROR",
      event.message,
      `${event.filename || "<unknown>"}:${event.lineno}:${event.colno}`,
      event.error
    );
  });

  window.addEventListener("unhandledrejection", (event) => {
    debugLog("WINDOW:UNHANDLED_REJECTION", event.reason);
  });

  document.addEventListener("visibilitychange", () => {
    debugLog("LIFECYCLE", `visibility=${document.visibilityState}`);
    if (document.visibilityState === "hidden") flush();
  });

  window.addEventListener("pagehide", () => {
    debugLog("LIFECYCLE", "pagehide");
    flush();
  });
  window.addEventListener("pageshow", (event) => {
    debugLog("LIFECYCLE", `pageshow persisted=${event.persisted}`);
  });
  window.addEventListener("beforeunload", () => {
    debugLog("LIFECYCLE", "beforeunload");
    flush();
  });
  window.addEventListener("focus", () => debugLog("LIFECYCLE", "window focus"));
  window.addEventListener("blur", () => debugLog("LIFECYCLE", "window blur"));

  sessionId = `${Date.now()}-${Math.random().toString(16).slice(2, 10)}`;
  if (recoveredBreadcrumb) {
    debugLog("PHASE8_80_RECOVERED_LAST_BREADCRUMB", recoveredBreadcrumb);
  }
  if (recoveredNativeRing.length) {
    debugLog("PHASE8_81_RECOVERED_NATIVE_RING_BEGIN", `count=${recoveredNativeRing.length}`);
    for (let i = 0; i < recoveredNativeRing.length; i += 1) {
      debugLog("PHASE8_81_RECOVERED_NATIVE_RING", `index=${i}`, recoveredNativeRing[i]);
    }
    debugLog("PHASE8_81_RECOVERED_NATIVE_RING_END", `count=${recoveredNativeRing.length}`);
  }
  debugLog("PHASE8_81_NATIVE_RING_ARMED", `capacity=${NATIVE_RING_MAX}`);
  debugLog(
    "SYSTEM",
    "=== New WIPI Player session ===",
    `session=${sessionId}`,
    `userAgent=${navigator.userAgent}`,
    `viewport=${window.innerWidth}x${window.innerHeight}`,
    `dpr=${window.devicePixelRatio}`,
    `visibility=${document.visibilityState}`
  );
};
