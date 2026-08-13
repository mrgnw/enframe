import { describe, expect, test } from 'bun:test';
import { AUTO_LANDSCAPE_LABEL } from '../composition';
import { type CropDoc, CROP_DOC_VERSION, docFromSidecar, docToSidecar, emptyDoc, nextLocalId } from './types';

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
