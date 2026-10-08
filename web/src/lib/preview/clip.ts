// The clip, drawn in the browser. The sidecar is the contract: each photo's row says which part of it is the
// frame, and `placeFrame` does what the CLI's `place()` does with it, onto a canvas the size of the clip.

import type { SidecarPhoto } from '#lib/marking/types.js';

export type PadChoice = 'none' | 'black' | 'white' | 'transparent' | 'custom';
export type Format = 'avif' | 'mp4' | 'mov' | 'webm';

export const FORMATS: Format[] = ['avif', 'mp4', 'mov', 'webm'];
export const FPS_PRESETS = [2, 4, 8, 12];
export const FPS_DEFAULT = 4;
/** The longest side of a preview frame in pixels; the export is as large as the photos allow. */
export const PREVIEW_MAX = 1000;

const median = (values: number[]) => {
	const sorted = [...values].sort((a, b) => a - b);
	const n = sorted.length;
	return n % 2 ? sorted[(n - 1) / 2] : (sorted[n / 2 - 1] + sorted[n / 2]) / 2;
};

/** Rounds half to even, as the CLI does. */
const roundEven = (v: number) => (v % 1 === 0.5 ? 2 * Math.round(v / 2) : Math.round(v));

/** The clip's size in pixels, as the CLI sizes it: the first crop at the typical photo's scale, even. */
export function clipSize(rows: SidecarPhoto[]): { w: number; h: number } {
	const first = rows[0];
	if (!first.width || !first.height) throw new Error('the sidecar has no photo sizes');
	const typical = median(rows.map((r) => r.scale));
	const even = (v: number) => roundEven((v * typical) / 2) * 2;
	return { w: even((first.w * first.width) / first.scale), h: even((first.h * first.height) / first.scale) };
}

/** The preview frame's size: the clip scaled down to PREVIEW_MAX on its long side, never up. */
export function previewSize(clip: { w: number; h: number }): { w: number; h: number } {
	const k = Math.min(1, PREVIEW_MAX / Math.max(clip.w, clip.h));
	return { w: Math.round(clip.w * k), h: Math.round(clip.h * k) };
}

/** Where a photo lands in a clip of `w` by `h`: its crop rect (0–1 of the photo, may overhang) maps to the clip. */
export function destRect(row: SidecarPhoto, w: number, h: number): [number, number, number, number] {
	return [(-row.x / row.w) * w, (-row.y / row.h) * h, w / row.w, h / row.h];
}

/** One frame: the photo at its crop rect on a transparent canvas the size of the clip, the padding left empty. */
export function placeFrame(photo: ImageBitmap, row: SidecarPhoto, w: number, h: number): ImageBitmap {
	const canvas = new OffscreenCanvas(w, h);
	const ctx = canvas.getContext('2d')!;
	ctx.imageSmoothingQuality = 'high';
	const [dx, dy, dw, dh] = destRect(row, w, h);
	ctx.drawImage(photo, dx, dy, dw, dh);
	return canvas.transferToImageBitmap();
}

/** Where the pinned feature sits in the clip (0–1): the mean of the photo's points, through its crop. */
export function pinAt(row: SidecarPhoto): [number, number] | null {
	if (!row.points.length) return null;
	const mean = (i: 0 | 1) => row.points.reduce((s, p) => s + p[i], 0) / row.points.length;
	return [(mean(0) - row.x) / row.w, (mean(1) - row.y) / row.h];
}

/** The CLI's `--pad` value, or undefined for none. */
export const padArg = (pad: PadChoice, custom: string): string | undefined => (pad === 'none' ? undefined : pad === 'custom' ? custom : pad);

/** The fill behind the frames, or null where nothing shows through (none) or the checkerboard does (transparent). */
export const fillOf = (pad: PadChoice, custom: string): string | null =>
	pad === 'black' || pad === 'white' ? pad : pad === 'custom' ? custom : null;

/** Why a format cannot be chosen with this pad, or null. Only MP4 (H.264) has no alpha channel. */
export const formatBlocked = (format: Format, pad: PadChoice): string | null =>
	format === 'mp4' && pad === 'transparent' ? 'Transparent padding needs AVIF, MOV or WebM' : null;

/** The path without a clip extension, so the format picks it. */
export const stripExt = (path: string) => path.replace(/\.(avif|mp4|mov|webm)$/i, '');

export const baseName = (path: string) => path.replace(/\/+$/, '').split('/').pop() ?? path;
