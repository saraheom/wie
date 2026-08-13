
import { debugLog } from "./debug_log";

export interface ConfirmOptions {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  destructive?: boolean;
}

let pendingResolve: ((value: boolean) => void) | undefined;

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
  overlay.hidden = false;

  debugLog("UI", `confirmation opened: ${options.title}`);

  return new Promise<boolean>((resolve) => {
    pendingResolve = resolve;
  });
};

const settleConfirmation = (value: boolean) => {
  const overlay = document.getElementById("confirm-overlay");
  if (overlay) overlay.hidden = true;

  const resolve = pendingResolve;
  pendingResolve = undefined;
  resolve?.(value);
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
  byId("confirm-cancel").addEventListener("click", () => settleConfirmation(false));
  byId("confirm-accept").addEventListener("click", () => settleConfirmation(true));
  byId("confirm-overlay").addEventListener("click", (event) => {
    if (event.target === event.currentTarget) settleConfirmation(false);
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
