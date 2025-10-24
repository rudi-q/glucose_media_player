import { writable } from 'svelte/store';

export interface SetupStatus {
	ffmpeg_installed: boolean;
	models_installed: string[];
	setup_completed: boolean;
}

export interface AppSettings {
	subtitleLanguage: string;
	selectedAudioDevice: string;
	volume: number;
	isMuted: boolean;
}

const defaultSettings: AppSettings = {
	subtitleLanguage: 'auto',
	selectedAudioDevice: 'default',
	volume: 1,
	isMuted: false
};

function createAppStore() {
	const { subscribe, set, update } = writable<AppSettings>(defaultSettings);

	return {
		subscribe,
		updateSubtitleLanguage: (language: string) =>
			update((state) => ({ ...state, subtitleLanguage: language })),
		updateAudioDevice: (deviceId: string) =>
			update((state) => ({ ...state, selectedAudioDevice: deviceId })),
		updateVolume: (volume: number) =>
			update((state) => ({ ...state, volume })),
		updateMuted: (isMuted: boolean) =>
			update((state) => ({ ...state, isMuted })),
		reset: () => set(defaultSettings)
	};
}

export const appSettings = createAppStore();

// Setup status store
function createSetupStore() {
	const { subscribe, set } = writable<SetupStatus | null>(null);

	return {
		subscribe,
		setStatus: (status: SetupStatus) => set(status),
		reset: () => set(null)
	};
}

export const setupStore = createSetupStore();
