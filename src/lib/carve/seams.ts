import type { Energy } from './types';

export interface SeamRequest {
	w: number;
	h: number;
	luma: Float32Array;
	cost: Float32Array;
	energy: Energy;
	straight: number;
	blur: number;
	seams: number;
}

export function boxBlur(source: Float32Array, w: number, h: number, radius: number): Float32Array {
	const pass = (
		input: Float32Array,
		output: Float32Array,
		along: number,
		across: number,
		step: number,
		lineStep: number
	) => {
		for (let line = 0; line < across; line++) {
			const base = line * lineStep;
			let sum = 0;
			for (let i = -radius; i <= radius; i++) sum += input[base + Math.min(Math.max(i, 0), along - 1) * step];
			for (let i = 0; i < along; i++) {
				output[base + i * step] = sum / (2 * radius + 1);
				sum += input[base + Math.min(i + radius + 1, along - 1) * step] - input[base + Math.max(i - radius, 0) * step];
			}
		}
	};
	const temp = new Float32Array(w * h);
	const out = new Float32Array(w * h);
	pass(source, temp, w, h, 1, w);
	pass(temp, out, h, w, w, 1);
	return out;
}

// One seam per step, top to bottom, each reported in original columns. `luma` and `cost` are
// consumed: every removed seam closes up in place.
export function carveSeams(request: SeamRequest, emit: (step: number, seam: Int32Array) => void): void {
	const { w, h, energy, straight, seams, cost } = request;
	const luma = request.blur > 0 ? boxBlur(request.luma, w, h, request.blur) : request.luma;
	const source = new Int32Array(w * h);
	for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) source[y * w + x] = x;
	const total = new Float32Array(w * h);
	const back = new Int8Array(w * h);
	const seam = new Int32Array(h);
	const forward = energy === 'forward';
	let width = w;
	for (let step = 0; step < seams && width > 1; step++) {
		for (let y = 0; y < h; y++) {
			const row = y * w;
			const prev = row - w;
			for (let x = 0; x < width; x++) {
				const l = luma[row + (x > 0 ? x - 1 : x)];
				const r = luma[row + (x < width - 1 ? x + 1 : x)];
				const across = Math.abs(r - l);
				let up: number;
				let left: number;
				let right: number;
				if (forward) {
					const u = y > 0 ? luma[prev + x] : luma[row + x];
					up = across;
					left = across + Math.abs(u - l) + straight;
					right = across + Math.abs(u - r) + straight;
				} else {
					const u = luma[(y > 0 ? prev : row) + x];
					const d = luma[(y < h - 1 ? row + w : row) + x];
					up = across + Math.abs(d - u);
					left = up + straight;
					right = up + straight;
				}
				if (y === 0) {
					total[row + x] = up + cost[row + x];
					back[row + x] = 0;
					continue;
				}
				let best = total[prev + x] + up;
				let dir = 0;
				if (x > 0 && total[prev + x - 1] + left < best) {
					best = total[prev + x - 1] + left;
					dir = -1;
				}
				if (x < width - 1 && total[prev + x + 1] + right < best) {
					best = total[prev + x + 1] + right;
					dir = 1;
				}
				total[row + x] = best + cost[row + x];
				back[row + x] = dir;
			}
		}
		const last = (h - 1) * w;
		let x = 0;
		for (let i = 1; i < width; i++) if (total[last + i] < total[last + x]) x = i;
		for (let y = h - 1; y >= 0; y--) {
			seam[y] = x;
			x += back[y * w + x];
		}
		const original = new Int32Array(h);
		for (let y = 0; y < h; y++) {
			const row = y * w;
			const at = row + seam[y];
			original[y] = source[at];
			luma.copyWithin(at, at + 1, row + width);
			source.copyWithin(at, at + 1, row + width);
			cost.copyWithin(at, at + 1, row + width);
		}
		width--;
		emit(step, original);
	}
}
