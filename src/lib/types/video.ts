/**
 * Represents a video file in the filesystem.
 * @property {string} path - Absolute filesystem path to the video file
 * @property {string} name - Filename (e.g., "video.mp4")
 * @property {number} size - File size in bytes
 * @property {number} modified - Last modified timestamp in seconds since epoch (multiply by 1000 for Date)
 * @property {number} [duration] - Video duration in seconds (optional)
 * @property {boolean} is_cloud_only - true if the file is a cloud-only placeholder (not locally materialized)
 */
export interface VideoFile {
	path: string;
	name: string;
	size: number;
	modified: number;
	duration?: number;
	is_cloud_only: boolean;
}

/**
 * Common video format types
 */
export type VideoFormat = 'MP4' | 'MKV' | 'WEBM' | 'AVI' | 'MOV' | 'FLV' | 'WMV' | string;

/**
 * Video metadata information.
 * @property {VideoFormat} format - Video format as uppercase string token (e.g., 'MP4', 'WEBM', 'MKV')
 * @property {number} sizeMb - File size in megabytes
 */
export interface VideoInfo {
	format: VideoFormat;
	sizeMb: number;
}
