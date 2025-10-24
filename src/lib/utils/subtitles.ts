/**
 * Convert SRT subtitle format to WebVTT format
 */
export function convertSrtToVtt(srt: string): string {
	// Convert SRT to WebVTT format
	let vtt = 'WEBVTT\n\n';

	// Clean up the SRT content
	// Remove BOM if present
	let cleanSrt = srt.replace(/^\uFEFF/, '');

	// Replace SRT timestamps (00:00:00,000) with VTT timestamps (00:00:00.000)
	// SRT format: 00:00:20,000 --> 00:00:24,400
	// VTT format: 00:00:20.000 --> 00:00:24.400
	cleanSrt = cleanSrt.replace(/(\d{2}:\d{2}:\d{2}),(\d{3})/g, '$1.$2');

	vtt += cleanSrt;

	return vtt;
}

/**
 * Load subtitle file from path and return WebVTT blob URL
 */
export async function loadSubtitleFile(
	path: string
): Promise<{ blobUrl: string; fileName: string } | null> {
	try {
		if (import.meta.env.DEV) {
			console.log('=== LOADING SUBTITLE ===');
			console.log('Path:', path);
		}
		// Record subtitle file name for UI display
		const fileName = path.split(/[/\\]/).pop() || 'Subtitles';

		// Read the subtitle file content
		const { readTextFile } = await import('@tauri-apps/plugin-fs');
		const content = await readTextFile(path);

		if (import.meta.env.DEV) {
			console.log('Subtitle content loaded, length:', content.length);
			console.log('First 200 chars:', content.substring(0, 200));
		}

		// Determine subtitle format and handle accordingly
		let vttContent: string;
		const ext = path.toLowerCase().split('.').pop() || '';

		if (ext === 'vtt' || content.startsWith('WEBVTT')) {
			// Already in WebVTT format
			if (import.meta.env.DEV) console.log('Subtitle is WebVTT format');
			vttContent = content;
		} else if (ext === 'srt') {
			// Convert SRT to WebVTT
			if (import.meta.env.DEV) console.log('Converting SRT to WebVTT');
			vttContent = convertSrtToVtt(content);
			if (import.meta.env.DEV) console.log('WebVTT first 200 chars:', vttContent.substring(0, 200));
		} else if (ext === 'ass' || ext === 'ssa' || ext === 'sub') {
			// Reject unsupported formats gracefully
			console.error(`Unsupported subtitle format: ${ext}`);
			alert(
				`Sorry, ${ext.toUpperCase()} subtitle format is not yet supported.\n\nPlease convert your subtitles to SRT or VTT format.\n\nSupported formats: SRT, VTT`
			);
			return null;
		} else {
			// Unknown format - try to detect by content
			if (content.includes('[Script Info]') || content.includes('Dialogue:')) {
				console.error('Detected ASS/SSA format without proper extension');
				alert(
					'This appears to be an ASS/SSA subtitle file, which is not yet supported.\n\nPlease convert to SRT or VTT format.'
				);
			} else if (/^{\d+}{\d+}/.test(content)) {
				console.error('Detected MicroDVD format');
				alert(
					'This appears to be a MicroDVD subtitle file, which is not yet supported.\n\nPlease convert to SRT or VTT format.'
				);
			} else {
				console.error('Unknown subtitle format');
				alert('Unsupported subtitle file format.\n\nSupported formats: SRT, VTT');
			}
			return null;
		}

		// Create a blob URL from the content
		const blob = new Blob([vttContent], { type: 'text/vtt;charset=utf-8' });
		const blobUrl = URL.createObjectURL(blob);

		if (import.meta.env.DEV) {
			console.log('Subtitle blob URL created:', blobUrl);
			console.log('=== SUBTITLE LOADING COMPLETE ===');
		}

		return { blobUrl, fileName };
	} catch (err) {
		console.error('Failed to load subtitle:', err);
		alert('Failed to load subtitle file: ' + err);
		return null;
	}
}
