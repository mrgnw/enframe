import type { ExportFormat, Frame } from './types';

export interface Encoding {
	mime: string;
	extension: ExportFormat;
}

export function encodingFor(format: ExportFormat): Encoding {
	if (format === 'png') return { mime: 'image/png', extension: 'png' };
	if (format === 'jpg') return { mime: 'image/jpeg', extension: 'jpg' };
	return { mime: 'image/avif', extension: 'avif' };
}

// The app carries one 0–1 quality dial. The Rust exporter scales it to the 0–100 the
// image crate wants, and libavif in the browser wants the same 0–100, so both paths
// land on the same number.
export function avifQuality(quality: number): number {
	return clamp(Math.round(quality * 100), 0, 100);
}

// A browser that cannot start the encoder must abort the export rather than write a
// format nobody asked for, so pay the encoder's startup before the first file lands.
export async function prepareEncoder(encoding: Encoding): Promise<void> {
	if (encoding.extension !== 'avif') return;
	const { encodeAvif } = await import('./avif');
	await encodeAvif(new ImageData(1, 1), 100);
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

export function canvasBlob(canvas: HTMLCanvasElement, mime: string, quality: number): Promise<Blob> {
	return new Promise((resolve, reject) => {
		canvas.toBlob(
			(blob) => {
				if (blob) resolve(blob);
				else reject(new Error('Could not encode the crop'));
			},
			mime,
			quality
		);
	});
}

// ponytail: crops on the main thread with a plain canvas, and ignores frame rotation
// exactly as the Rust exporter does. Only the AVIF encode moves to a worker, because
// that is the part that takes seconds on a 45 MP crop.
export async function cropToBlob(
	image: HTMLImageElement,
	frame: Frame,
	encoding: Encoding,
	quality = 0.92
): Promise<Blob> {
	const full = { width: image.naturalWidth, height: image.naturalHeight };
	if (full.width === 0 || full.height === 0) throw new Error('Image has no dimensions');

	const x = clamp(Math.round(frame.x * full.width), 0, full.width - 1);
	const y = clamp(Math.round(frame.y * full.height), 0, full.height - 1);
	const width = clamp(Math.round(frame.width * full.width), 1, full.width - x);
	const height = clamp(Math.round(frame.height * full.height), 1, full.height - y);

	const canvas = document.createElement('canvas');
	canvas.width = width;
	canvas.height = height;
	const context = canvas.getContext('2d');
	if (!context) throw new Error('Canvas 2D is unavailable');
	context.drawImage(image, x, y, width, height, 0, 0, width, height);

	if (encoding.extension !== 'avif') return canvasBlob(canvas, encoding.mime, quality);

	const { encodeAvif } = await import('./avif');
	const buffer = await encodeAvif(context.getImageData(0, 0, width, height), avifQuality(quality));
	return new Blob([buffer], { type: encoding.mime });
}
