/**
 * Format seconds as HH:MM:SS or MM:SS
 */
export function formatTime(seconds: number): string {
	const h = Math.floor(seconds / 3600);
	const m = Math.floor((seconds % 3600) / 60);
	const s = Math.floor(seconds % 60);

	if (h > 0) {
		return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
	}
	return `${m}:${s.toString().padStart(2, '0')}`;
}

/**
 * Format duration from seconds (same as formatTime)
 */
export function formatDuration(seconds?: number): string {
	if (!seconds) return '';
	return formatTime(seconds);
}

/**
 * Format estimated time in a human-readable way
 */
export function formatEstimatedTime(seconds: number): string {
	if (seconds < 60) {
		return `~${Math.round(seconds)}s`;
	} else if (seconds < 3600) {
		const mins = Math.round(seconds / 60);
		return `~${mins}m`;
	} else {
		const hours = Math.floor(seconds / 3600);
		const mins = Math.round((seconds % 3600) / 60);
		return mins > 0 ? `~${hours}h ${mins}m` : `~${hours}h`;
	}
}

/**
 * Format time for screen readers (aria-valuetext)
 */
export function formatTimeForScreenReader(seconds: number): string {
	const h = Math.floor(seconds / 3600);
	const m = Math.floor((seconds % 3600) / 60);
	const s = Math.floor(seconds % 60);
	
	const parts: string[] = [];
	if (h > 0) parts.push(`${h} ${h === 1 ? 'hour' : 'hours'}`);
	if (m > 0) parts.push(`${m} ${m === 1 ? 'minute' : 'minutes'}`);
	if (s > 0 || parts.length === 0) parts.push(`${s} ${s === 1 ? 'second' : 'seconds'}`);
	
	return parts.join(' ');
}
