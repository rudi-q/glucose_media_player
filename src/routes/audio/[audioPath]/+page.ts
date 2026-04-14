export const prerender = false;

export function load({ params }: { params: { audioPath: string } }) {
	return {
		audioPath: decodeURIComponent(params.audioPath)
	};
}
