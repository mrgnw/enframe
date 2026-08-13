import type { FileFormat } from './output';
import type { ExportFormat, Frame } from './types';

export interface Encoding {
	mime: string;
	extension: FileFormat;
	// True when the browser cannot encode the format the user asked for.
	substituted: boolean;
}

// No browser encodes AVIF from a canvas; asking for it silently yields PNG, so ask
// for WebP instead and let the caller say what happened.
export function encodingFor(format: ExportFormat): Encoding {
	if (format === 'png') return { mime: 'image/png', extension: 'png', substituted: false };
	if (format === 'jpg') return { mime: 'image/jpeg', extension: 'jpg', substituted: false };
	return { mime: 'image/webp', extension: 'webp', substituted: true };
}

export function loadImage(src: string): Promise<HTMLImageElement> {
	return new Promise((resolve, reject) => {
		const image = new Image();
		image.onload = () => resolve(image);
		image.onerror = () => reject(new Error('Could not decode the image'));
		image.src = src;
	});
}

function clamp(value: number, low: number, high: number): number {
	return Math.min(high, Math.max(low, value));
}

// ponytail: crops on the main thread with a plain canvas, and ignores frame rotation
// exactly as the Rust exporter does. Move to OffscreenCanvas in a worker if encoding
// a large export ever blocks the UI long enough to notice.
export function cropToBlob(image: HTMLImageElement, frame: Frame, encoding: Encoding, quality = 0.92): Promise<Blob> {
	const full = { width: image.naturalWidth, height: image.naturalHeight };
	if (full.width === 0 || full.height === 0) return Promise.reject(new Error('Image has no dimensions'));

	const x = clamp(Math.round(frame.x * full.width), 0, full.width - 1);
	const y = clamp(Math.round(frame.y * full.height), 0, full.height - 1);
	const width = clamp(Math.round(frame.width * full.width), 1, full.width - x);
	const height = clamp(Math.round(frame.height * full.height), 1, full.height - y);

	const canvas = document.createElement('canvas');
	canvas.width = width;
	canvas.height = height;
	const context = canvas.getContext('2d');
	if (!context) return Promise.reject(new Error('Canvas 2D is unavailable'));
	context.drawImage(image, x, y, width, height, 0, 0, width, height);

	return new Promise((resolve, reject) => {
		canvas.toBlob(
			(blob) => {
				if (blob) resolve(blob);
				else reject(new Error('Could not encode the crop'));
			},
			encoding.mime,
			quality
		);
	});
}
