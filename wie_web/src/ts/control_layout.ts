export type GameControlPreset = "classic" | "custom";
export type ControlPadName = "direction" | "number";

export const CONTROL_KEYS = [
  "UP", "LEFT", "OK", "RIGHT", "DOWN", "CLR",
  "1", "2", "3", "4", "5", "6", "7", "8", "9", "*", "0", "#",
] as const;

export type ControlKey = (typeof CONTROL_KEYS)[number];

export interface ControlPadLayout {
  /** Center position as a percentage of the player-content rectangle. */
  x: number;
  y: number;
  /** Visual scale, where 1 is the normal keypad size. */
  scale: number;
  /** CSS grid gap in pixels before scaling. */
  gap: number;
}

export interface ControlLayout {
  direction: ControlPadLayout;
  number: ControlPadLayout;
  /** Shared key opacity from 0.25 to 1. */
  opacity: number;
  /** Keys hidden only for this orientation. */
  hiddenKeys: ControlKey[];
}

export interface GameControlLayouts {
  portrait: ControlLayout;
  landscape: ControlLayout;
}

const pad = (x: number, y: number, scale = 1, gap = 10): ControlPadLayout => ({
  x,
  y,
  scale,
  gap,
});

export const defaultControlLayout = (orientation: "portrait" | "landscape"): ControlLayout =>
  orientation === "landscape"
    ? {
        direction: pad(14, 50, 1, 10),
        number: pad(86, 50, 1, 10),
        opacity: 1,
        hiddenKeys: [],
      }
    : {
        direction: pad(25, 82, 1, 8),
        number: pad(75, 82, 1, 8),
        opacity: 1,
        hiddenKeys: [],
      };

export const defaultControlLayouts = (): GameControlLayouts => ({
  portrait: defaultControlLayout("portrait"),
  landscape: defaultControlLayout("landscape"),
});

const finite = (value: unknown, fallback: number): number =>
  typeof value === "number" && Number.isFinite(value) ? value : fallback;

const clamp = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, value));

const normalizePad = (
  candidate: Partial<ControlPadLayout> | undefined,
  fallback: ControlPadLayout
): ControlPadLayout => ({
  x: clamp(finite(candidate?.x, fallback.x), 4, 96),
  y: clamp(finite(candidate?.y, fallback.y), 6, 94),
  scale: clamp(finite(candidate?.scale, fallback.scale), 0.6, 1.5),
  gap: clamp(finite(candidate?.gap, fallback.gap), 2, 24),
});

export const normalizeControlLayout = (
  candidate: Partial<ControlLayout> | undefined,
  orientation: "portrait" | "landscape"
): ControlLayout => {
  const fallback = defaultControlLayout(orientation);
  const hiddenKeys = Array.isArray(candidate?.hiddenKeys)
    ? candidate.hiddenKeys.filter((key): key is ControlKey =>
        CONTROL_KEYS.includes(key as ControlKey)
      )
    : [];

  return {
    direction: normalizePad(candidate?.direction, fallback.direction),
    number: normalizePad(candidate?.number, fallback.number),
    opacity: clamp(finite(candidate?.opacity, fallback.opacity), 0.25, 1),
    hiddenKeys: Array.from(new Set(hiddenKeys)),
  };
};

export const normalizeControlLayouts = (
  candidate?: Partial<GameControlLayouts>
): GameControlLayouts => ({
  portrait: normalizeControlLayout(candidate?.portrait, "portrait"),
  landscape: normalizeControlLayout(candidate?.landscape, "landscape"),
});

export const controlPresetLayout = (
  orientation: "portrait" | "landscape",
  preset: "classic" | "spacious" | "compact"
): ControlLayout => {
  const layout = defaultControlLayout(orientation);

  if (preset === "spacious") {
    layout.direction.scale = 1.06;
    layout.number.scale = 1.06;
    layout.direction.gap = 14;
    layout.number.gap = 14;
    if (orientation === "portrait") {
      layout.direction.x = 24;
      layout.number.x = 76;
    }
  } else if (preset === "compact") {
    layout.direction.scale = 0.82;
    layout.number.scale = 0.82;
    layout.direction.gap = 6;
    layout.number.gap = 6;
  }

  return layout;
};
