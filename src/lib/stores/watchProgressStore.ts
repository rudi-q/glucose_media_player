import { writable } from 'svelte/store';

export interface WatchProgress {
	path: string;
	current_time: number;
	duration: number;
	last_watched: number;
}

function createWatchProgressStore() {
	const { subscribe, set, update } = writable<Map<string, WatchProgress>>(new Map());

	return {
		subscribe,
		setProgress: (videoPath: string, progress: WatchProgress) =>
			update((map) => {
				const newMap = new Map(map);
				newMap.set(videoPath, progress);
				return newMap;
			}),
		loadAllProgress: (progressData: Record<string, WatchProgress>) =>
			set(new Map(Object.entries(progressData))),
		getProgress: (videoPath: string): WatchProgress | undefined => {
			let result: WatchProgress | undefined;
			subscribe((map) => {
				result = map.get(videoPath);
			})();
			return result;
		},
		clear: () => set(new Map())
	};
}

export const watchProgressStore = createWatchProgressStore();
