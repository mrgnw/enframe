import type { SeamReply } from './carver-worker';
import type { SeamRequest } from './seams';
import { type CarvePoint, type CarveSettings, MAX_REMOVED } from './types';

// A plane is ABGR pixels as the canvas hands them out, row-major.
export interface Plane {
	w: number;
	h: number;
	pixels: Uint32Array;
}

export interface Work {
	w: number;
	h: number;
}

export interface Band {
	from: number;
	to: number;
}

export type Rgb = [number, number, number];

const BLOCKED = 2e4;
const REMOVE = -2e4;

export const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

// ---- orientation. Shorter carves the transposed photo by columns, so everything below works on
// columns; only drawing and pointer input swap the axes back.

export function transpose<T extends Uint32Array | Float32Array | Uint16Array>(source: T, w: number, h: number): T {
	const out = new (source.constructor as new (n: number) => T)(w * h);
	for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) out[x * h + y] = source[y * w + x];
	return out;
}

export function orient(plane: Plane, rows: boolean): Plane {
	return rows ? { w: plane.h, h: plane.w, pixels: transpose(plane.pixels, plane.w, plane.h) } : plane;
}

export const orientPoint = (p: { x: number; y: number }, rows: boolean) =>
	rows ? { x: p.y, y: p.x } : { x: p.x, y: p.y };

export function workSize(width: number, height: number, working: number): Work {
	const scale = Math.min(1, working / Math.max(width, height));
	return { w: Math.max(2, Math.round(width * scale)), h: Math.max(1, Math.round(height * scale)) };
}

export function lumaOf(pixels: Uint32Array): Float32Array {
	const luma = new Float32Array(pixels.length);
	for (let i = 0; i < pixels.length; i++) {
		const p = pixels[i];
		luma[i] = 0.299 * (p & 255) + 0.587 * ((p >> 8) & 255) + 0.114 * ((p >> 16) & 255);
	}
	return luma;
}

// Per working pixel: the region block, the lean ramp, and every point's disc.
export function costMap(work: Work, settings: CarveSettings, points: CarvePoint[], rows: boolean): Float32Array {
	const { w, h } = work;
	const { from, to, lean } = settings;
	const cost = new Float32Array(w * h);
	for (let y = 0; y < h; y++) {
		for (let x = 0; x < w; x++) {
			const across = x / (w - 1);
			let c = -lean * 3 * (across * 2 - 1);
			if (across < from || across > to) c += BLOCKED;
			cost[y * w + x] = c;
		}
	}
	for (const point of points) {
		const o = orientPoint(point, rows);
		const cx = o.x * w;
		const cy = o.y * h;
		const radius = point.size * Math.min(w, h);
		const add = point.kind === 'remove' ? REMOVE : point.strength === 0 ? 0 : 2 ** point.strength;
		for (let y = Math.max(0, Math.floor(cy - radius)); y < Math.min(h, Math.ceil(cy + radius)); y++) {
			for (let x = Math.max(0, Math.floor(cx - radius)); x < Math.min(w, Math.ceil(cx + radius)); x++) {
				if ((x - cx) ** 2 + (y - cy) ** 2 <= radius * radius) cost[y * w + x] += add;
			}
		}
	}
	return cost;
}

export const seamsFor = (work: Work) => Math.round(MAX_REMOVED * work.w);

// What a size asks for: n seams removed, or the first e seams doubled.
export function target(size: number, work: Work, maxSeams: number): { n: number; e: number } {
	const n = size < 1 ? Math.min(Math.round((1 - size) * work.w), maxSeams) : 0;
	const e = size > 1 ? Math.min(Math.round((size - 1) * work.w), maxSeams) : 0;
	return { n, e };
}

export interface ColumnMap {
	map: Float32Array;
	length: number;
	marked: Int32Array;
}

// For every working row, the source column of each output column. A doubled seam adds a column
// half a pixel to its right, which samples between it and its neighbour. `marked` is where the
// nth seam sits in the output, for drawing it.
export function columnMap(rank: Uint16Array, work: Work, n: number, e: number): ColumnMap {
	const { w, h } = work;
	const length = w - n + e;
	const map = new Float32Array(h * length);
	const marked = new Int32Array(h).fill(-1);
	for (let y = 0; y < h; y++) {
		let i = 0;
		const row = y * w;
		const out = y * length;
		for (let x = 0; x < w; x++) {
			const r = rank[row + x];
			if (r === n) marked[y] = i;
			if (r < n) continue;
			map[out + i++] = x;
			if (r < e) map[out + i++] = Math.min(x + 0.5, w - 1);
		}
		if (i !== length) throw new Error(`row ${y} has ${i} columns, expected ${length}`);
	}
	return { map, length, marked };
}

// ---- pixels

export function imageDataOf(plane: Plane): ImageData {
	return new ImageData(new Uint8ClampedArray(plane.pixels.buffer as ArrayBuffer), plane.w, plane.h);
}

export function lerpPixel(a: number, b: number, f: number): number {
	if (f === 0 || a === b) return a;
	const g = 1 - f;
	const r = (a & 255) * g + (b & 255) * f;
	const gr = ((a >> 8) & 255) * g + ((b >> 8) & 255) * f;
	const bl = ((a >> 16) & 255) * g + ((b >> 16) & 255) * f;
	return 0xff000000 | (bl << 16) | (gr << 8) | r;
}

export function tint(pixel: number, [r, g, b]: Rgb, amount = 1): number {
	const k = 0.6 * amount;
	const mix = (c: number, t: number) => (c * (1 - k) + t * k) | 0;
	return 0xff000000 | (mix((pixel >> 16) & 255, b) << 16) | (mix((pixel >> 8) & 255, g) << 8) | mix(pixel & 255, r);
}

export const shade = (pixel: number) => 0xff000000 | ((pixel & 0xfefefe) >> 1);

export const outputWidth = (source: Plane, work: Work, length: number) =>
	Math.max(1, Math.round((source.w * length) / work.w));

// The carved photo at the source's resolution: the working column map is looked up bilinearly,
// so the seams become a smooth squeeze rather than blocky cuts. Columns outside `band` are
// shaded, for the preview.
export function renderResult(source: Plane, work: Work, layout: ColumnMap, band: Band | null = null): Plane {
	const { map, length } = layout;
	const { w, h } = work;
	const { w: dw, h: dh, pixels } = source;
	const outW = outputWidth(source, work, length);
	const out = new Uint32Array(outW * dh);
	for (let Y = 0; Y < dh; Y++) {
		const yl = clamp(((Y + 0.5) * h) / dh - 0.5, 0, h - 1);
		const y0 = yl | 0;
		const y1 = Math.min(y0 + 1, h - 1);
		const fy = yl - y0;
		const r0 = y0 * length;
		const r1 = y1 * length;
		const row = Y * dw;
		const target = Y * outW;
		for (let X = 0; X < outW; X++) {
			const il = clamp(((X + 0.5) * length) / outW - 0.5, 0, length - 1);
			const i0 = il | 0;
			const i1 = Math.min(i0 + 1, length - 1);
			const fi = il - i0;
			const m =
				(map[r0 + i0] * (1 - fi) + map[r0 + i1] * fi) * (1 - fy) + (map[r1 + i0] * (1 - fi) + map[r1 + i1] * fi) * fy;
			const sx = clamp(((m + 0.5) * dw) / w - 0.5, 0, dw - 1);
			const x0 = sx | 0;
			let p = lerpPixel(pixels[row + x0], pixels[row + Math.min(x0 + 1, dw - 1)], sx - x0);
			if (band) {
				const across = (m + 0.5) / w;
				if (across < band.from || across > band.to) p = shade(p);
			}
			out[target + X] = p;
		}
	}
	return { w: outW, h: dh, pixels: out };
}

// Where an oriented original position lands across the result, 0–1: the share of columns still
// standing to its left in its working row. Null when its own column is gone.
export function resultFraction(
	o: { x: number; y: number },
	rank: Uint16Array,
	work: Work,
	n: number,
	e: number
): number | null {
	const { w, h } = work;
	const row = clamp(Math.round(o.y * h - 0.5), 0, h - 1) * w;
	const px = clamp(o.x * w, 0, w - 1);
	if (rank[row + (px | 0)] < n) return null;
	let count = 0;
	for (let x = 0; x < px; x++) {
		const r = rank[row + x];
		if (r >= n) count += r < e ? 2 : 1;
	}
	return count / (w - n + e);
}

// A focal point carried into the version, or null when the carve removed what it marked.
export function mapPoint(
	point: { x: number; y: number },
	rank: Uint16Array,
	work: Work,
	n: number,
	e: number,
	rows: boolean
): { x: number; y: number } | null {
	const o = orientPoint(point, rows);
	const x = resultFraction(o, rank, work, n, e);
	if (x === null) return null;
	return orientPoint({ x, y: o.y }, rows);
}

// ---- the worker. One per carve; terminating it is the cancel.

export function startCarve(
	request: SeamRequest,
	onSeam: (step: number, seam: Int32Array, elapsed: number) => void,
	onDone: (elapsed: number) => void
): () => void {
	const worker = new Worker(new URL('./carver-worker.ts', import.meta.url), { type: 'module' });
	worker.onmessage = ({ data }: MessageEvent<SeamReply>) => {
		if ('done' in data) {
			onDone(data.elapsed);
			worker.terminate();
		} else onSeam(data.step, data.seam, data.elapsed);
	};
	worker.postMessage(request, [request.luma.buffer, request.cost.buffer]);
	return () => worker.terminate();
}
