import { writable, derived } from 'svelte/store';
import { SUBTITLE_PRESETS, DEFAULT_PRESET_ID, type SubtitleStyle } from './subtitlePresets';

const STORAGE_KEY = 'glucose_subtitle_style';

interface StyleState {
  presetId: string;
  customizations: Partial<Omit<SubtitleStyle, 'id' | 'name'>>;
}

const BG_TYPES = ['none', 'pill', 'box', 'stripe', 'frosted'] as const;

function sanitizeCustomizations(raw: unknown): Partial<Omit<SubtitleStyle, 'id' | 'name'>> {
  if (!raw || typeof raw !== 'object') return {};
  const obj = raw as Record<string, unknown>;
  const result: Partial<Omit<SubtitleStyle, 'id' | 'name'>> = {};
  for (const key of ['fontSize', 'position', 'lineHeight'] as const) {
    if (typeof obj[key] === 'number' && isFinite(obj[key] as number)) result[key] = obj[key] as number;
  }
  for (const key of ['fontFamily', 'fontWeight', 'color', 'backgroundColor', 'textShadow', 'textStroke', 'letterSpacing'] as const) {
    if (typeof obj[key] === 'string') result[key] = obj[key] as string;
  }
  if (BG_TYPES.includes(obj.backgroundType as typeof BG_TYPES[number])) {
    result.backgroundType = obj.backgroundType as typeof BG_TYPES[number];
  }
  return result;
}

function loadFromStorage(): StyleState {
  const fallback: StyleState = { presetId: DEFAULT_PRESET_ID, customizations: {} };
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw);
    if (
      parsed &&
      typeof parsed === 'object' &&
      typeof parsed.presetId === 'string' &&
      SUBTITLE_PRESETS.some((p) => p.id === parsed.presetId)
    ) {
      return { presetId: parsed.presetId, customizations: sanitizeCustomizations(parsed.customizations) };
    }
    return fallback;
  } catch {
    return fallback;
  }
}

function createSubtitleStyleStore() {
  const { subscribe, set, update } = writable<StyleState>(loadFromStorage());

  function saveToStorage(state: StyleState) {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
    } catch {
      // ignore storage errors
    }
  }

  return {
    subscribe,
    setPreset(id: string) {
      const valid = SUBTITLE_PRESETS.some((p) => p.id === id);
      const next = { presetId: valid ? id : DEFAULT_PRESET_ID, customizations: {} };
      saveToStorage(next);
      set(next);
    },
    customize(overrides: Partial<Omit<SubtitleStyle, 'id' | 'name'>>) {
      update((current) => {
        const next = { ...current, customizations: { ...current.customizations, ...sanitizeCustomizations(overrides) } };
        saveToStorage(next);
        return next;
      });
    },
    reset() {
      const next = { presetId: DEFAULT_PRESET_ID, customizations: {} };
      saveToStorage(next);
      set(next);
    },
  };
}

export const subtitleStyleStore = createSubtitleStyleStore();

export const activeSubtitleStyle = derived(subtitleStyleStore, ($store) => {
  const base = SUBTITLE_PRESETS.find((p) => p.id === $store.presetId) ?? SUBTITLE_PRESETS[0];
  return { ...base, ...$store.customizations };
});
