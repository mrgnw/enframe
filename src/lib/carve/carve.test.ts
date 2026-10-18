import { describe, expect, test } from 'vitest';
import { columnMap, costMap, mapPoint, renderResult, target, transpose } from './carve';
import { carveSeams } from './seams';
import { type CarvePoint, DEFAULT_SETTINGS } from './types';

const W = 24;
const H = 12;

function carve(luma: Float32Array, cost: Float32Array, seams: number): Uint16Array {
	const rank = new Uint16Array(W * H).fill(0xffff);
	carveSeams({ w: W, h: H, luma, cost, energy: 'forward', straight: 0, blur: 0, seams }, (step, seam) => {
		for (let y = 0; y < H; y++) rank[y * W + seam[y]] = step;
	});
	return rank;
}

const flat = () => new Float32Array(W * H).fill(100);
const free = () => new Float32Array(W * H);
const work = { w: W, h: H };

describe('carveSeams', () => {
	test('every row loses exactly one column per seam', () => {
		const rank = carve(flat(), free(), 10);
		for (let y = 0; y < H; y++) {
			let removed = 0;
			for (let x = 0; x < W; x++) if (rank[y * W + x] < 10) removed++;
			expect(removed).toBe(10);
		}
	});

	test('a protected disc is never crossed', () => {
		const disc: CarvePoint = { kind: 'protect', x: 0.5, y: 0.5, size: 0.3, strength: 10 };
		const rank = carve(flat(), costMap(work, DEFAULT_SETTINGS, [disc], false), 14);
		const radius = 0.3 * H;
		for (let y = 0; y < H; y++) {
			for (let x = 0; x < W; x++) {
				if ((x - W / 2) ** 2 + (y - H / 2) ** 2 <= radius * radius) expect(rank[y * W + x]).toBe(0xffff);
			}
		}
	});

	test('a remove disc goes first', () => {
		const disc: CarvePoint = { kind: 'remove', x: 0.75, y: 0.5, size: 0.2, strength: 10 };
		const rank = carve(flat(), costMap(work, DEFAULT_SETTINGS, [disc], false), 1);
		const y = H / 2;
		const removed = [...rank.subarray(y * W, (y + 1) * W)].indexOf(0);
		expect(Math.abs(removed - 0.75 * W)).toBeLessThanOrEqual(0.2 * H);
	});
});

describe('columnMap and renderResult', () => {
	test('the output is exactly as wide as asked, at any scale', () => {
		const rank = carve(flat(), free(), 10);
		const { n, e } = target(0.75, work, 14);
		expect(n).toBe(6);
		const layout = columnMap(rank, work, n, e);
		expect(layout.length).toBe(W - 6);
		const source = { w: 96, h: 48, pixels: new Uint32Array(96 * 48).fill(0xff336699) };
		const out = renderResult(source, work, layout);
		expect(out.w).toBe(72);
		expect(out.h).toBe(48);
		expect(out.pixels[0]).toBe(0xff336699);
	});

	test('widening doubles the first seams', () => {
		const rank = carve(flat(), free(), 10);
		const { n, e } = target(1.25, work, 14);
		expect(columnMap(rank, work, n, e).length).toBe(W + 6);
	});
});

describe('mapPoint', () => {
	test('stays put when nothing left of it is removed, and is gone inside a removed seam', () => {
		const rank = carve(flat(), free(), 4);
		const kept = mapPoint({ x: 0.9, y: 0.5 }, rank, work, 4, 0, false);
		const row = (H / 2) * W;
		let standing = 0;
		for (let x = 0; x < 0.9 * W; x++) if (rank[row + x] >= 4) standing++;
		expect(kept?.x).toBeCloseTo(standing / (W - 4));
		const removedX = [...rank.subarray(row, row + W)].findIndex((r) => r < 4);
		expect(mapPoint({ x: (removedX + 0.5) / W, y: 0.5 }, rank, work, 4, 0, false)).toBeNull();
	});

	test('rows mode maps through the transpose', () => {
		const rank = carve(flat(), free(), 0);
		const point = mapPoint({ x: 0.25, y: 0.5 }, rank, work, 0, 0, true);
		expect(point).toEqual({ x: 0.25, y: 0.5 });
	});
});

test('transpose round-trips', () => {
	const a = new Uint32Array([1, 2, 3, 4, 5, 6]);
	expect([...transpose(transpose(a, 3, 2), 2, 3)]).toEqual([...a]);
});
