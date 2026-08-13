import { canvasBlob } from './canvas-export';

// Matches the extensions the Rust inbox scanner accepts, so a file the desktop app
// would ingest is a file the browser will try to develop.
export const RAW_EXTENSIONS = ['arw', 'cr2', 'cr3', 'dng', 'nef', 'raf'] as const;

export function isRawName(name: string): boolean {
	const dot = name.lastIndexOf('.');
	if (dot < 0) return false;
	const extension = name.slice(dot + 1).toLowerCase();
	return RAW_EXTENSIONS.some((raw) => raw === extension);
}

// ponytail: the same 90% gate as `preview_is_full_size` in src-tauri/src/commands.rs,
// integer maths and all, so the two decoders agree on which files take the fast path.
export function previewIsFullSize(previewWidth: number, rawWidth: number): boolean {
	return previewWidth * 10 >= rawWidth * 9;
}

export interface RawPreview {
	width: number;
	format: string;
}

// Canon CR2/CR3, Nikon NEF and most DNG carry a full-size JPEG the camera already
// rendered. Sony ARW previews cap at 1616x1080, so they fail this and get demosaiced.
export function usePreview(preview: RawPreview | null, rawWidth: number): boolean {
	if (!preview || preview.format !== 'jpeg') return false;
	return previewIsFullSize(preview.width, rawWidth);
}

function message(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

// ponytail: RGB8 straight from LibRaw, repacked to RGBA for a canvas. Three full-frame
// buffers exist at once here — the source, the RGBA copy and the canvas — so a 45 MP
// file peaks around half a gigabyte. Stream it in tiles if that ever becomes a problem.
function pixelsToBlob(width: number, height: number, rgb: Uint8Array): Promise<Blob> {
	const rgba = new Uint8ClampedArray(width * height * 4);
	for (let target = 0, source = 0; target < rgba.length; target += 4, source += 3) {
		rgba[target] = rgb[source];
		rgba[target + 1] = rgb[source + 1];
		rgba[target + 2] = rgb[source + 2];
		rgba[target + 3] = 255;
	}

	const canvas = document.createElement('canvas');
	canvas.width = width;
	canvas.height = height;
	const context = canvas.getContext('2d');
	if (!context) throw new Error('Canvas 2D is unavailable');
	context.putImageData(new ImageData(rgba, width, height), 0, 0);
	return canvasBlob(canvas, 'image/png', 1);
}

// Decodes a camera RAW into something an <img> can render. LibRaw runs in the worker the
// package ships, and the wasm only downloads once a RAW is actually opened.
export async function decodeRaw(file: File): Promise<Blob> {
	const { default: LibRaw } = await import('libraw-wasm');
	const decoder = new LibRaw();
	try {
		await decoder.open(new Uint8Array(await file.arrayBuffer()), { useCameraWb: true });

		const meta = await decoder.metadata();
		if (!meta) throw new Error('no RAW metadata');
		const rawWidth = meta.raw_width || meta.width;

		const preview = await decoder.thumbnailData().catch(() => undefined);
		if (preview && usePreview(preview, rawWidth)) {
			return new Blob([preview.data as BlobPart], { type: 'image/jpeg' });
		}

		const image = await decoder.imageData();
		if (!image) throw new Error('the decoder returned no pixels');
		if (image.colors !== 3 || image.bits !== 8) {
			throw new Error(`unexpected ${image.bits}-bit ${image.colors}-channel output`);
		}
		return pixelsToBlob(image.width, image.height, image.data as Uint8Array);
	} catch (error) {
		throw new Error(`Could not develop ${file.name} — ${message(error)}`);
	} finally {
		decoder.dispose();
	}
}
