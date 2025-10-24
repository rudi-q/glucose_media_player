export function load({ params }: { params: { videoPath: string } }) {
	return {
		videoPath: decodeURIComponent(params.videoPath)
	};
}
