import { describe, expect, test } from 'vitest';
import { isRawName, previewIsFullSize, usePreview } from './raw';

describe('previewIsFullSize', () => {
	// The same numbers the Rust tests in src-tauri/src/commands.rs assert on, so the two
	// decoders provably agree on the gate.
	test('a Sony ARW preview falls through to the develop path', () => {
		expect(previewIsFullSize(1616, 6024)).toBe(false);
	});

	test('a full-width preview is taken', () => {
		expect(previewIsFullSize(6024, 6024)).toBe(true);
	});

	test('the boundary sits exactly at 90 percent', () => {
		expect(previewIsFullSize(5422, 6024)).toBe(true);
		expect(previewIsFullSize(5421, 6024)).toBe(false);
	});

	test('a missing preview width never passes', () => {
		expect(previewIsFullSize(0, 6024)).toBe(false);
	});
});

describe('isRawName', () => {
	test('accepts every extension the Rust inbox accepts', () => {
		for (const name of ['a.arw', 'a.cr2', 'a.cr3', 'a.dng', 'a.nef', 'a.raf']) {
			expect(isRawName(name)).toBe(true);
		}
	});

	test('ignores case', () => {
		expect(isRawName('sample_00019.ARW')).toBe(true);
		expect(isRawName('shot.Cr3')).toBe(true);
	});

	test('rejects the formats a browser already decodes', () => {
		for (const name of ['a.jpg', 'a.jpeg', 'a.png', 'a.webp', 'a.avif', 'a.tif', 'a.json']) {
			expect(isRawName(name)).toBe(false);
		}
	});

	test('only the extension counts', () => {
		expect(isRawName('arw')).toBe(false);
		expect(isRawName('my.arw.jpg')).toBe(false);
		expect(isRawName('nef-backup/holiday.png')).toBe(false);
		expect(isRawName('')).toBe(false);
		expect(isRawName('trailing.')).toBe(false);
	});
});

describe('usePreview', () => {
	test('takes a full-size embedded JPEG', () => {
		expect(usePreview({ width: 6000, format: 'jpeg' }, 6024)).toBe(true);
	});

	test('demosaics when the preview is too small', () => {
		expect(usePreview({ width: 1616, format: 'jpeg' }, 6024)).toBe(false);
	});

	test('demosaics when there is no preview at all', () => {
		expect(usePreview(null, 6024)).toBe(false);
	});

	test('demosaics when the preview is not a JPEG we can hand to an img', () => {
		expect(usePreview({ width: 6000, format: 'bitmap' }, 6024)).toBe(false);
		expect(usePreview({ width: 6000, format: 'unknown' }, 6024)).toBe(false);
	});
});
