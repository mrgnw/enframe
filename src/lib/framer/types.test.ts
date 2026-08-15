import { describe, expect, test } from 'vitest';
import { AUTO_LANDSCAPE_LABEL } from '../composition';
import {
	type CropDoc,
	CROP_DOC_VERSION,
	cycleFrameId,
	docFromSidecar,
	docToSidecar,
	emptyDoc,
	framesAtPoint,
	nextLocalId,
	nextSelection
} from './types';

const DOC: CropDoc = {
	version: CROP_DOC_VERSION,
	image: { name: 'beach.jpg', width: 4000, height: 3000 },
	frames: [
		{
			id: 7,
			label: 'Desktop',
			destination: 'desktop',
			x: 0.1,
			y: 0.2,
			width: 0.5,
			height: 0.625,
			rotation: 0,
			is_default: false,
			is_auto: true
		},
		{
			id: -1,
			label: undefined,
			destination: null,
			x: 0,
			y: 0,
			width: 1,
			height: 1,
			rotation: 0,
			is_default: false,
			is_auto: false
		}
	],
	focalPoints: [
		{ id: 3, frame_id: 7, x: 0.4, y: 0.45, label: 'Eyes', required: true },
		{ id: -2, frame_id: undefined, x: 0.8, y: 0.1, label: undefined, required: false }
	]
};

describe('sidecar serialization', () => {
	test('round-trips a document, negative ids included', () => {
		expect(docFromSidecar(docToSidecar(DOC, '/photos/beach.jpg'))).toEqual(DOC);
	});

	test('keeps the on-disk field names', () => {
		const sidecar = docToSidecar(DOC, '/photos/beach.jpg');
		expect(sidecar.version).toBe(CROP_DOC_VERSION);
		expect(sidecar.photo).toEqual({
			title: 'beach.jpg',
			width: 4000,
			height: 3000,
			file_path: '/photos/beach.jpg'
		});
		expect(sidecar.focal_points).toHaveLength(2);
		expect(sidecar.frames[0].destination).toBe('desktop');
	});

	test('reads a sidecar written before the version field', () => {
		const doc = docFromSidecar({
			photo: { title: 'old.jpg', width: 100, height: 50 },
			frames: [
				{
					id: 1,
					label: AUTO_LANDSCAPE_LABEL,
					destination: null,
					is_auto: false,
					x: 0,
					y: 0,
					width: 1,
					height: 1,
					rotation: 0,
					is_default: false
				}
			]
		});
		expect(doc.version).toBe(CROP_DOC_VERSION);
		expect(doc.image).toEqual({ name: 'old.jpg', width: 100, height: 50 });
		expect(doc.frames[0].is_auto).toBe(true);
		expect(doc.focalPoints).toEqual([]);
	});

	test('an empty sidecar yields an empty document', () => {
		expect(docFromSidecar({})).toEqual(emptyDoc());
	});
});

describe('cycleFrameId', () => {
	const ids = DOC.frames.map((f) => f.id);

	test('wraps forwards and backwards', () => {
		expect(cycleFrameId(DOC.frames, ids[0], 1)).toBe(ids[1]);
		expect(cycleFrameId(DOC.frames, ids[1], 1)).toBe(ids[0]);
		expect(cycleFrameId(DOC.frames, ids[0], -1)).toBe(ids[1]);
		expect(cycleFrameId(DOC.frames, ids[1], -1)).toBe(ids[0]);
	});

	test('starts at either end when nothing is selected', () => {
		expect(cycleFrameId(DOC.frames, null, 1)).toBe(ids[0]);
		expect(cycleFrameId(DOC.frames, null, -1)).toBe(ids[1]);
	});

	test('recovers from an id that is no longer in the list', () => {
		expect(cycleFrameId(DOC.frames, 999, 1)).toBe(ids[0]);
		expect(cycleFrameId(DOC.frames, 999, -1)).toBe(ids[0]);
	});

	test('has nothing to select in an empty document', () => {
		expect(cycleFrameId([], null, 1)).toBeNull();
		expect(cycleFrameId([], 7, -1)).toBeNull();
	});
});

describe('framesAtPoint', () => {
	// Frame 7 covers x 0.1-0.6, y 0.2-0.825; frame -1 covers the whole image and paints last.
	test('returns every frame under the point, topmost first', () => {
		expect(framesAtPoint(DOC.frames, 0.3, 0.4).map((f) => f.id)).toEqual([-1, 7]);
	});

	test('skips frames the point falls outside of', () => {
		expect(framesAtPoint(DOC.frames, 0.9, 0.9).map((f) => f.id)).toEqual([-1]);
	});

	test('counts the edges as inside', () => {
		expect(framesAtPoint(DOC.frames, 0.1, 0.2).map((f) => f.id)).toEqual([-1, 7]);
		expect(framesAtPoint(DOC.frames, 0.6, 0.825).map((f) => f.id)).toEqual([-1, 7]);
	});

	test('finds nothing outside the image', () => {
		expect(framesAtPoint(DOC.frames, 1.5, 0.5)).toEqual([]);
		expect(framesAtPoint([], 0.5, 0.5)).toEqual([]);
	});

	test('cycles through the stack under one point and wraps', () => {
		const hits = framesAtPoint(DOC.frames, 0.3, 0.4);
		expect(cycleFrameId(hits, null, 1)).toBe(-1);
		expect(cycleFrameId(hits, -1, 1)).toBe(7);
		expect(cycleFrameId(hits, 7, 1)).toBe(-1);
	});

	test('a selection outside the stack lands on the topmost frame', () => {
		const hits = framesAtPoint(DOC.frames, 0.9, 0.9);
		expect(cycleFrameId(hits, 7, 1)).toBe(-1);
	});
});

describe('nextLocalId', () => {
	test('starts at -1 for a fresh document', () => {
		expect(nextLocalId(emptyDoc())).toBe(-1);
	});

	test('stays below every id already in the document', () => {
		expect(nextLocalId(DOC)).toBe(-3);
	});

	test('never collides with positive database ids', () => {
		const doc = { ...emptyDoc(), frames: DOC.frames.slice(0, 1) };
		expect(nextLocalId(doc)).toBe(-1);
	});
});

describe('nextSelection', () => {
	test('a plain click replaces the selection and anchors on the clicked frame', () => {
		const result = nextSelection(new Set([1, 2, 3]), 5, 'replace');
		expect([...result.selection]).toEqual([5]);
		expect(result.anchor).toBe(5);
	});

	test('toggling adds a frame and anchors on it', () => {
		const result = nextSelection(new Set([1]), 2, 'toggle');
		expect([...result.selection]).toEqual([1, 2]);
		expect(result.anchor).toBe(2);
	});

	test('toggling a selected frame off falls back to the last one still selected', () => {
		const result = nextSelection(new Set([1, 2, 3]), 3, 'toggle');
		expect([...result.selection]).toEqual([1, 2]);
		expect(result.anchor).toBe(2);
	});

	test('toggling the last frame off leaves nothing selected', () => {
		const result = nextSelection(new Set([7]), 7, 'toggle');
		expect(result.selection.size).toBe(0);
		expect(result.anchor).toBeNull();
	});

	test('toggling into an empty selection starts one', () => {
		const result = nextSelection(new Set(), 4, 'toggle');
		expect([...result.selection]).toEqual([4]);
		expect(result.anchor).toBe(4);
	});

	test('leaves the previous selection untouched', () => {
		const previous = new Set([1, 2]);
		nextSelection(previous, 3, 'toggle');
		expect([...previous]).toEqual([1, 2]);
	});
});
