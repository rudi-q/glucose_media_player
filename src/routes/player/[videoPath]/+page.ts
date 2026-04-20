export const prerender = false;

export function load({ params, url }: { params: { videoPath: string }; url: URL }) {
	return {
		videoPath: decodeURIComponent(params.videoPath),
		initialMode: url.searchParams.get('mode') ?? 'cinematic'
	};
}
