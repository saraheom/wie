
import { debugLog } from "./debug_log";

export interface ConfirmOptions {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  destructive?: boolean;
}

let pendingResolve: ((value: boolean) => void) | undefined;
const describeTarget = (target: EventTarget | null) => {
  const el = target instanceof Element ? target : null;
  if (!el) return "<none>";
  const id = el.id ? `#${el.id}` : "";
  const cls = el.className && typeof el.className === "string" ? `.${el.className.trim().replace(/\s+/g, ".")}` : "";
  return `${el.tagName.toLowerCase()}${id}${cls}`;
};

const logConfirmationInput = (event: Event) => {
  const overlay = document.getElementById("confirm-overlay");
  if (!overlay || overlay.hidden) return;
  const pointer = event as PointerEvent;
  const x = Number.isFinite(pointer.clientX) ? pointer.clientX : -1;
  const y = Number.isFinite(pointer.clientY) ? pointer.clientY : -1;
  const hit = x >= 0 && y >= 0 ? document.elementFromPoint(x, y) : null;
  const accept = document.getElementById("confirm-accept")?.getBoundingClientRect();
  const cancel = document.getElementById("confirm-cancel")?.getBoundingClientRect();
  debugLog(
    "UI:CONFIRM_INPUT",
    `type=${event.type}`,
    `target=${describeTarget(event.target)}`,
    `xy=${x},${y}`,
    `hit=${describeTarget(hit)}`,
    `defaultPrevented=${event.defaultPrevented}`,
    `accept=${accept ? `${accept.left.toFixed(0)},${accept.top.toFixed(0)},${accept.width.toFixed(0)}x${accept.height.toFixed(0)}` : "missing"}`,
    `cancel=${cancel ? `${cancel.left.toFixed(0)},${cancel.top.toFixed(0)},${cancel.width.toFixed(0)}x${cancel.height.toFixed(0)}` : "missing"}`
  );
};


const byId = <T extends HTMLElement = HTMLElement>(id: string): T => {
  const found = document.getElementById(id);
  if (!found) throw new Error(`Missing element #${id}`);
  return found as T;
};

export const showToast = (
  message: string,
  kind: "normal" | "success" | "error" = "normal",
  duration = 2200
) => {
  const toast = document.getElementById("ui-toast");
  if (!toast) return;

  toast.textContent = message;
  toast.dataset.kind = kind;
  toast.classList.remove("visible");
  void toast.offsetWidth;
  toast.classList.add("visible");
  window.setTimeout(() => toast.classList.remove("visible"), duration);
};

export const requestConfirmation = (options: ConfirmOptions): Promise<boolean> => {
  if (pendingResolve) {
    pendingResolve(false);
    pendingResolve = undefined;
  }

  const overlay = byId("confirm-overlay");
  const title = byId("confirm-title");
  const message = byId("confirm-message");
  const confirm = byId<HTMLButtonElement>("confirm-accept");
  const cancel = byId<HTMLButtonElement>("confirm-cancel");

  title.textContent = options.title;
  message.textContent = options.message;
  confirm.textContent = options.confirmLabel ?? "Confirm";
  cancel.textContent = options.cancelLabel ?? "Cancel";
  confirm.classList.toggle("danger-action", Boolean(options.destructive));
  overlay.style.pointerEvents = "auto";
  overlay.hidden = false;

  debugLog("UI", `confirmation opened: ${options.title}`);

  return new Promise<boolean>((resolve) => {
    pendingResolve = resolve;
  });
};

const settleConfirmation = (value: boolean) => {
  if (!pendingResolve) return;
  debugLog("UI", `confirmation settled: ${value ? "confirm" : "cancel"}`);

  const overlay = document.getElementById("confirm-overlay");
  if (overlay) {
    overlay.hidden = true;
    overlay.style.pointerEvents = "none";
  }

  const resolve = pendingResolve;
  pendingResolve = undefined;
  resolve(value);
};

const bestEffortHaptic = (strength: "tap" | "success" | "warning" = "tap") => {
  try {
    if (!navigator.vibrate) return;
    if (strength === "success") navigator.vibrate([12, 20, 12]);
    else if (strength === "warning") navigator.vibrate(20);
    else navigator.vibrate(8);
  } catch {
    // iOS WKWebView usually does not expose the Vibration API.
  }
};

export const initUiFeedback = () => {
  debugLog("UI", "initUiFeedback installed");
  for (const type of ["pointerdown", "pointerup", "touchstart", "touchend", "click"] as const) {
    document.addEventListener(type, logConfirmationInput, { capture: true, passive: true });
  }

  const cancel = byId<HTMLButtonElement>("confirm-cancel");
  const accept = byId<HTMLButtonElement>("confirm-accept");
  const overlay = byId("confirm-overlay");

  // iOS WKWebView can suppress the synthetic click after a touch/pointer sequence,
  // especially when the emulator canvas has active gesture handlers. Resolve the
  // dialog directly on pointerup and keep click as keyboard/accessibility fallback.
  const bindConfirmButton = (button: HTMLButtonElement, value: boolean) => {
    button.addEventListener("pointerup", (event) => {
      event.preventDefault();
      event.stopPropagation();
      settleConfirmation(value);
    });
    button.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      settleConfirmation(value);
    });
  };

  bindConfirmButton(cancel, false);
  bindConfirmButton(accept, true);
  overlay.addEventListener("pointerup", (event) => {
    if (event.target === event.currentTarget) {
      event.preventDefault();
      event.stopPropagation();
      settleConfirmation(false);
    }
  });

  document.addEventListener(
    "pointerdown",
    (event) => {
      const button = (event.target as Element | null)?.closest?.("button") as HTMLButtonElement | null;
      if (!button || button.disabled) return;
      button.classList.add("ui-pressed");
      bestEffortHaptic("tap");
    },
    { passive: true }
  );

  const release = (event: Event) => {
    const button = (event.target as Element | null)?.closest?.("button") as HTMLButtonElement | null;
    button?.classList.remove("ui-pressed");
  };

  document.addEventListener("pointerup", release, { passive: true });
  document.addEventListener("pointercancel", release, { passive: true });
  document.addEventListener("pointerleave", release, { passive: true });

  document.addEventListener("click", (event) => {
    const button = (event.target as Element | null)?.closest?.("button") as HTMLButtonElement | null;
    if (!button || button.disabled) return;

    button.classList.remove("ui-action-flash");
    void button.offsetWidth;
    button.classList.add("ui-action-flash");
    window.setTimeout(() => button.classList.remove("ui-action-flash"), 260);
  });
};

export const successFeedback = (message: string) => {
  bestEffortHaptic("success");
  showToast(message, "success");
};

export const errorFeedback = (message: string) => {
  bestEffortHaptic("warning");
  showToast(message, "error", 3200);
};
