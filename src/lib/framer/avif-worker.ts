import encode from '@jsquash/avif/encode';

export interface AvifRequest {
	id: number;
	data: Uint8ClampedArray<ArrayBuffer>;
	width: number;
	height: number;
	quality: number;
}

export interface AvifReply {
	id: number;
	buffer?: ArrayBuffer;
	error?: string;
}

declare function postMessage(message: AvifReply): void;

self.onmessage = async (event: MessageEvent<AvifRequest>) => {
	const { id, data, width, height, quality } = event.data;
	try {
		const buffer = await encode(new ImageData(data, width, height), { quality });
		postMessage({ id, buffer });
	} catch (error) {
		postMessage({ id, error: error instanceof Error ? error.message : 'AVIF encoding failed' });
	}
};
