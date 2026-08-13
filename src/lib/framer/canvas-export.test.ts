import { describe, expect, test } from 'bun:test';
import { avifQuality, encodingFor, prepareEncoder } from './canvas-export';
import type { ExportFormat } from './types';

const FORMATS: ExportFormat[] = ['avif', 'jpg', 'png'];

describe('encodingFor', () => {
	test('gives every format its own mime type and extension', () => {
		expect(encodingFor('avif')).toEqual({ mime: 'image/avif', extension: 'avif' });
		expect(encodingFor('jpg')).toEqual({ mime: 'image/jpeg', extension: 'jpg' });
		expect(encodingFor('png')).toEqual({ mime: 'image/png', extension: 'png' });
	});

	test('never substitutes a different format for the one that was asked for', () => {
		for (const format of FORMATS) {
			const encoding = encodingFor(format);
			expect(encoding.extension).toBe(format);
			expect(encoding.mime.endsWith(format === 'jpg' ? 'jpeg' : format)).toBe(true);
		}
	});
});

describe('avifQuality', () => {
	test('scales the 0–1 dial onto the encoder 0–100 scale', () => {
		expect(avifQuality(0.92)).toBe(92);
		expect(avifQuality(0)).toBe(0);
		expect(avifQuality(1)).toBe(100);
		expect(avifQuality(0.505)).toBe(51);
	});

	test('clamps quality that falls outside the dial', () => {
		expect(avifQuality(-2)).toBe(0);
		expect(avifQuality(4)).toBe(100);
	});
});

describe('prepareEncoder', () => {
	// The canvas formats must not pay for the wasm encoder — this resolves without
	// touching it, and would throw here if it tried (no Worker, no DOM under bun).
	test('only the AVIF path loads an encoder', async () => {
		await prepareEncoder(encodingFor('jpg'));
		await prepareEncoder(encodingFor('png'));
	});
});
