/**
 * Represents a video file in the filesystem.
 * @property {string} path - Absolute filesystem path to the video file
 * @property {string} name - Filename (e.g., "video.mp4")
 * @property {number} size - File size in bytes
 * @property {number} modified - Last modified timestamp in epoch milliseconds
 * @property {number} [duration] - Video duration in seconds (optional)
 */
export interface VideoFile {
	path: string;
	name: string;
	size: number;
	modified: number;
	duration?: number;
}

/**
 * Video metadata information.
 * @property {string} format - Video format as uppercase string token (e.g., 'MP4', 'WEBM', 'MKV')
 * @property {number} sizeMb - File size in megabytes
 */
export interface VideoInfo {
	format: string;
	sizeMb: number;
}
