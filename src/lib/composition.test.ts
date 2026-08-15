import { describe, expect, test } from 'vitest';
import {
	computeAutoFrame,
	containsPoint,
	excludedRequiredPoints,
	type FocalInput,
	objectPosition,
	type Rect
} from './composition';

const IMAGE_ASPECT = 3 / 2;

const RATIOS = [16 / 9, 9 / 16, 1, 4 / 5, 1.91, 3 / 2, 2 / 3, 21 / 9];

function pixelAspect(rect: Rect, imageAspect: number): number {
	return (rect.width * imageAspect) / rect.height;
}

function inBounds(rect: Rect): boolean {
	return (
		rect.x >= 0 &&
		rect.y >= 0 &&
		rect.width > 0 &&
		rect.height > 0 &&
		rect.x + rect.width <= 1 + 1e-9 &&
		rect.y + rect.height <= 1 + 1e-9
	);
}

describe('computeAutoFrame ratios', () => {
	const points: FocalInput[] = [
		{ x: 0.4, y: 0.45, required: true },
		{ x: 0.55, y: 0.5 }
	];

	for (const ratio of RATIOS) {
		test(`ratio ${ratio.toFixed(4)} produces an in-bounds rect of that ratio`, () => {
			for (const imageAspect of [3 / 2, 2 / 3, 1]) {
				const result = computeAutoFrame(points, ratio, imageAspect);
				expect(inBounds(result.rect)).toBe(true);
				expect(pixelAspect(result.rect, imageAspect)).toBeCloseTo(ratio, 6);
			}
		});
	}
});

describe('required point constraint', () => {
	test('every required point stays inside the computed rect', () => {
		const points: FocalInput[] = [
			{ x: 0.4, y: 0.35, required: true },
			{ x: 0.6, y: 0.6, required: true }
		];
		for (const ratio of RATIOS) {
			const result = computeAutoFrame(points, ratio, IMAGE_ASPECT);
			expect(result.satisfiesRequired).toBe(true);
			expect(excludedRequiredPoints(result.rect, points)).toHaveLength(0);
		}
	});

	test('optional points may be cropped out without failing the fit', () => {
		const points: FocalInput[] = [
			{ x: 0.5, y: 0.5, required: true },
			{ x: 0.99, y: 0.01 }
		];
		const result = computeAutoFrame(points, 1, IMAGE_ASPECT);
		expect(result.satisfiesRequired).toBe(true);
		expect(containsPoint(result.rect, points[0])).toBe(true);
	});

	test('impossible constraint returns best effort and flags it', () => {
		// Two required points on opposite corners cannot fit a narrow portrait crop.
		const points: FocalInput[] = [
			{ x: 0.01, y: 0.5, required: true },
			{ x: 0.99, y: 0.5, required: true }
		];
		const result = computeAutoFrame(points, 9 / 16, IMAGE_ASPECT);
		expect(result.satisfiesRequired).toBe(false);
		expect(inBounds(result.rect)).toBe(true);
		expect(excludedRequiredPoints(result.rect, points).length).toBeGreaterThan(0);
	});
});

describe('degenerate inputs', () => {
	test('no focal points returns a centered max-size rect', () => {
		const result = computeAutoFrame([], 16 / 9, IMAGE_ASPECT);
		expect(result.satisfiesRequired).toBe(true);
		expect(inBounds(result.rect)).toBe(true);
		expect(pixelAspect(result.rect, IMAGE_ASPECT)).toBeCloseTo(16 / 9, 6);
		expect(result.rect.x + result.rect.width / 2).toBeCloseTo(0.5, 6);
		expect(result.rect.y + result.rect.height / 2).toBeCloseTo(0.5, 6);
	});

	test('a required point at the image corner is still contained', () => {
		const corners: FocalInput[][] = [
			[{ x: 0, y: 0, required: true }],
			[{ x: 1, y: 1, required: true }],
			[{ x: 0, y: 1, required: true }]
		];
		for (const points of corners) {
			const result = computeAutoFrame(points, 4 / 5, IMAGE_ASPECT);
			expect(result.satisfiesRequired).toBe(true);
			expect(containsPoint(result.rect, points[0])).toBe(true);
		}
	});

	test('square image and square target fills the frame', () => {
		const result = computeAutoFrame([{ x: 0.5, y: 0.5, required: true }], 1, 1);
		expect(result.rect.width).toBeCloseTo(result.rect.height, 6);
	});
});

describe('crop and zoom trade-off', () => {
	test('a centered point keeps more of the image than the minimum crop', () => {
		const result = computeAutoFrame([{ x: 0.5, y: 0.5, required: true }], 3 / 2, IMAGE_ASPECT);
		expect(result.rect.width * result.rect.height).toBeGreaterThan(0.5);
	});

	test('scoring is deterministic', () => {
		const points: FocalInput[] = [{ x: 0.33, y: 0.66, required: true }];
		const a = computeAutoFrame(points, 16 / 9, IMAGE_ASPECT);
		const b = computeAutoFrame(points, 16 / 9, IMAGE_ASPECT);
		expect(a.rect).toEqual(b.rect);
		expect(a.score).toBe(b.score);
	});
});

describe('objectPosition', () => {
	test('full-width crop centers horizontally', () => {
		expect(objectPosition({ x: 0, y: 0.25, width: 1, height: 0.5 })).toBe('50.0% 50.0%');
	});

	test('crop pinned to an edge maps to 0% / 100%', () => {
		expect(objectPosition({ x: 0, y: 0, width: 0.5, height: 0.5 })).toBe('0.0% 0.0%');
		expect(objectPosition({ x: 0.5, y: 0.5, width: 0.5, height: 0.5 })).toBe('100.0% 100.0%');
	});
});
