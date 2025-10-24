export interface VideoFile {
	path: string;
	name: string;
	size: number;
	modified: number;
	duration?: number;
}

export interface VideoInfo {
	format: string;
	sizeMb: number;
}
