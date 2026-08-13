const STORAGE_KEY = "wipi_player_debug_log_v1";
const MAX_LINES = 2500;
const MAX_CHARS = 700_000;

let lines: string[] = [];
let flushTimer: number | undefined;
let installed = false;

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

const trimLog = () => {
  if (lines.length > MAX_LINES) lines = lines.slice(lines.length - MAX_LINES);

  let chars = lines.reduce((sum, line) => sum + line.length + 1, 0);
  while (chars > MAX_CHARS && lines.length > 1) {
    const removed = lines.shift();
    chars -= (removed?.length ?? 0) + 1;
  }
};

const flush = () => {
  flushTimer = undefined;
  try {
    localStorage.setItem(STORAGE_KEY, lines.join("\n"));
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
  lines.push(`[${timestamp}] [${category}] ${payload}`.trimEnd());
  trimLog();
  scheduleFlush();
};

export const getDebugLogText = () => lines.join("\n");

export const clearDebugLog = () => {
  lines = [];
  try {
    localStorage.removeItem(STORAGE_KEY);
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

  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) lines = stored.split("\n").filter(Boolean);
  } catch {
    lines = [];
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

  window.addEventListener("pagehide", flush);

  debugLog(
    "SYSTEM",
    "=== New WIPI Player session ===",
    `userAgent=${navigator.userAgent}`,
    `viewport=${window.innerWidth}x${window.innerHeight}`,
    `dpr=${window.devicePixelRatio}`
  );
};
