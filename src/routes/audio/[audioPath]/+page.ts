export const prerender = false;

export function load({ params }: { params: { audioPath: string } }) {
	return {
		// SvelteKit has already decoded the param; decoding again breaks names containing '%'.
		audioPath: params.audioPath
	};
}
