import { writable, derived } from 'svelte/store';
import { SUBTITLE_PRESETS, DEFAULT_PRESET_ID, type SubtitleStyle } from './subtitlePresets';

const STORAGE_KEY = 'glucose_subtitle_style';

interface StyleState {
  presetId: string;
  customizations: Partial<Omit<SubtitleStyle, 'id' | 'name'>>;
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
      parsed.customizations !== null &&
      typeof parsed.customizations === 'object' &&
      SUBTITLE_PRESETS.some((p) => p.id === parsed.presetId)
    ) {
      return parsed as StyleState;
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
        const next = { ...current, customizations: { ...current.customizations, ...overrides } };
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
