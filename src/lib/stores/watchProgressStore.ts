import { writable, get } from 'svelte/store';

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
		const map = get({ subscribe });
		return map.get(videoPath);
	},
		clear: () => set(new Map()),
		clearBefore: (cutoff: number) => update((map) => {
			if (cutoff === 0) return new Map();
			const next = new Map<string, WatchProgress>();
			for (const [key, val] of map) {
				if (val.last_watched < cutoff) next.set(key, val);
			}
			return next;
		}),
	};
}

export const watchProgressStore = createWatchProgressStore();
