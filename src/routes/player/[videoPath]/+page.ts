export const prerender = false;

export function load({ params, url }: { params: { videoPath: string }; url: URL }) {
	return {
		// SvelteKit has already decoded the param; decoding again breaks names containing '%'.
		videoPath: params.videoPath,
		initialMode: url.searchParams.get('mode') ?? 'cinematic',
		restart: url.searchParams.get('restart') === 'true'
	};
}
