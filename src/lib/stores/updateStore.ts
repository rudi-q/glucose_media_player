import { writable } from 'svelte/store';

export interface UpdateState {
	checking: boolean;
	available: boolean;
	downloading: boolean;
	downloaded: number;
	contentLength: number;
	version?: string;
	date?: string;
	body?: string;
	error?: string;
	completed: boolean;
	upToDate: boolean;
}

const initialState: UpdateState = {
	checking: false,
	available: false,
	downloading: false,
	downloaded: 0,
	contentLength: 0,
	completed: false,
	upToDate: false
};

function createUpdateStore() {
	const { subscribe, set, update } = writable<UpdateState>(initialState);

	return {
		subscribe,
		reset: () => set(initialState),
	setChecking: (checking: boolean) => update((state) => ({
		...state,
		checking,
		// Clear stale availability and metadata when starting a new check
		available: false,
		version: undefined,
		date: undefined,
		body: undefined,
		upToDate: false,
		error: undefined,
		downloaded: 0,
		contentLength: 0,
		completed: false
	})),
	setAvailable: (available: boolean, version?: string, date?: string, body?: string) =>
		update((state) => ({
			...state,
			available,
			// Clear metadata when available is false to prevent stale data
			version: available ? version : undefined,
			date: available ? date : undefined,
			body: available ? body : undefined,
			upToDate: false,
			// Clear checking and error flags when final availability is set
			checking: false,
			error: undefined
		})),
	setDownloading: (downloading: boolean) => update((state) => ({
		...state,
		downloading,
		// Clear error when starting a download
		error: downloading ? undefined : state.error
	})),
		setProgress: (downloaded: number, contentLength: number) =>
			update((state) => ({ ...state, downloaded, contentLength })),
	setCompleted: (completed: boolean) => update((state) => ({
		...state,
		completed,
		// Clear downloading flag when completed
		downloading: false
	})),
		setError: (error: string) =>
			update((state) => ({
			...state,
			error,
			checking: false,
			downloading: false,
			upToDate: false,
			available: false,
			version: undefined,
			date: undefined,
			body: undefined,
			downloaded: 0,
			contentLength: 0,
			completed: false
		})),
	setUpToDate: (upToDate: boolean) => update((state) => ({
		...state,
		upToDate,
		// Clear availability and metadata when up to date
		available: upToDate ? false : state.available,
		version: upToDate ? undefined : state.version,
		date: upToDate ? undefined : state.date,
		body: upToDate ? undefined : state.body
	}))
	};
}

export const updateStore = createUpdateStore();
