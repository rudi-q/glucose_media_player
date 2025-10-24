/**
 * Convert SRT subtitle format to WebVTT format
 */
export function convertSrtToVtt(srt: string): string {
	// Convert SRT to WebVTT format
	let vtt = 'WEBVTT\n\n';

	// Clean up the SRT content
	// Normalize line endings (CRLF/CR to LF) first
	let cleanSrt = srt.replace(/\r\n?/g, '\n');
	
	// Remove BOM if present
	cleanSrt = cleanSrt.replace(/^\uFEFF/, '');

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

		// Normalize content: remove BOM and normalize line endings
		const normalizedContent = content.replace(/^\uFEFF/, '').replace(/\r\n?/g, '\n');

		if (import.meta.env.DEV) {
			console.log('Normalized first 200 chars:', normalizedContent.substring(0, 200));
		}

	// Determine subtitle format by checking content first, then extension
	let vttContent: string;
	const ext = path.toLowerCase().split('.').pop() || '';

	// Check content markers first (priority: WebVTT, then SRT patterns)
	if (normalizedContent.startsWith('WEBVTT')) {
		// Already in WebVTT format (regardless of extension)
		if (import.meta.env.DEV) console.log('Detected WebVTT format by content');
		vttContent = normalizedContent;
	} else if (
		// Detect SRT by content: timestamp pattern and sequence numbers
		/^\d+\s*\n\d{2}:\d{2}:\d{2}[,.]\d{3}\s*-->\s*\d{2}:\d{2}:\d{2}[,.]\d{3}/m.test(normalizedContent)
	) {
		// SRT format detected by content (regardless of extension)
		if (import.meta.env.DEV) console.log('Detected SRT format by content, converting to WebVTT');
		vttContent = convertSrtToVtt(normalizedContent);
		if (import.meta.env.DEV) console.log('WebVTT first 200 chars:', vttContent.substring(0, 200));
	} else if (normalizedContent.includes('[Script Info]') || normalizedContent.includes('Dialogue:')) {
		// ASS/SSA format detected by content
		console.error('Detected ASS/SSA format by content');
		alert(
			'This appears to be an ASS/SSA subtitle file, which is not yet supported.\n\nPlease convert to SRT or VTT format.'
		);
		return null;
	} else if (/^{\d+}{\d+}/.test(normalizedContent)) {
		// MicroDVD format detected by content
		console.error('Detected MicroDVD format by content');
		alert(
			'This appears to be a MicroDVD subtitle file, which is not yet supported.\n\nPlease convert to SRT or VTT format.'
		);
		return null;
	} else if (ext === 'vtt') {
		// Extension suggests VTT but content doesn't match - assume it's valid VTT
		if (import.meta.env.DEV) console.log('Treating as WebVTT based on extension');
		vttContent = normalizedContent;
	} else if (ext === 'srt') {
		// Extension suggests SRT but no clear pattern - try conversion anyway
		if (import.meta.env.DEV) console.log('Treating as SRT based on extension, attempting conversion');
		vttContent = convertSrtToVtt(normalizedContent);
	} else if (ext === 'ass' || ext === 'ssa' || ext === 'sub') {
		// Extension-based rejection for unsupported formats
		console.error(`Unsupported subtitle format by extension: ${ext}`);
		alert(
			`Sorry, ${ext.toUpperCase()} subtitle format is not yet supported.\n\nPlease convert your subtitles to SRT or VTT format.\n\nSupported formats: SRT, VTT`
		);
		return null;
	} else {
		// Unknown format - neither content nor extension match known formats
		console.error('Unknown subtitle format (content and extension)');
		alert('Unsupported subtitle file format.\n\nSupported formats: SRT, VTT');
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
