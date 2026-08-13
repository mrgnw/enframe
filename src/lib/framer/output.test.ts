import { describe, expect, test } from 'bun:test';
import { basename, generateFrameFilename, getDefaultExportDir, sidecarJson, sidecarName } from './output';
import { type CropDoc, CROP_DOC_VERSION, docFromSidecar, type Frame } from './types';

function frame(patch: Partial<Frame>): Frame {
	return {
		id: 1,
		label: undefined,
		destination: null,
		x: 0,
		y: 0,
		width: 0.5,
		height: 0.5,
		rotation: 0,
		is_default: false,
		is_auto: false,
		...patch
	};
}

const PHOTO = { title: 'A Sunny Day.jpg' };

describe('generateFrameFilename', () => {
	test('prefers the destination slug', () => {
		const name = generateFrameFilename(PHOTO, frame({ destination: 'desktop', label: 'Desktop' }), 0, 'jpg');
		expect(name).toBe('A-Sunny-Day.desktop.1.00.jpg');
	});

	test('falls back to the slugified label', () => {
		const name = generateFrameFilename(PHOTO, frame({ label: 'Hero Banner!' }), 0, 'png');
		expect(name).toBe('A-Sunny-Day.hero-banner.1.00.png');
	});

	test('falls back to orientation and index', () => {
		expect(generateFrameFilename(PHOTO, frame({ width: 0.8, height: 0.4 }), 0, 'avif')).toBe(
			'A-Sunny-Day.l.1.2.00.avif'
		);
		expect(generateFrameFilename(PHOTO, frame({ width: 0.2, height: 0.4 }), 1, 'avif')).toBe(
			'A-Sunny-Day.p.2.0.50.avif'
		);
		expect(generateFrameFilename(PHOTO, frame({ width: 0.4, height: 0.4 }), 2, 'avif')).toBe(
			'A-Sunny-Day.s.3.1.00.avif'
		);
	});

	test('de-dupes names that collide within one export', () => {
		const used = new Set<string>();
		const twins = [frame({ label: 'Crop' }), frame({ label: 'Crop' }), frame({ label: 'Crop' })];
		const names = twins.map((f, i) => generateFrameFilename(PHOTO, f, i, 'jpg', used));
		expect(names).toEqual(['A-Sunny-Day.crop.1.00.jpg', 'A-Sunny-Day.crop-2.1.00.jpg', 'A-Sunny-Day.crop-3.1.00.jpg']);
		expect(new Set(names).size).toBe(3);
	});

	test('names an untitled photo', () => {
		expect(generateFrameFilename({}, frame({}), 0, 'jpg')).toBe('photo.s.1.1.00.jpg');
	});
});

describe('paths', () => {
	test('basename drops the extension and the spaces', () => {
		expect(basename('beach walk.JPEG')).toBe('beach-walk');
		expect(basename('')).toBe('photo');
	});

	test('the sidecar sits beside the image', () => {
		expect(sidecarName('beach walk.jpg')).toBe('beach-walk.metadata.json');
	});

	test('the default export dir is the image folder', () => {
		expect(getDefaultExportDir('/photos/trip/beach.jpg')).toBe('/photos/trip');
		expect(getDefaultExportDir('beach.jpg')).toBe('/');
	});
});

describe('standalone sidecar', () => {
	const DOC: CropDoc = {
		version: CROP_DOC_VERSION,
		image: { name: 'beach.jpg', width: 4000, height: 3000 },
		frames: [
			frame({
				id: -1,
				label: 'Desktop',
				destination: 'desktop',
				x: 0.1,
				y: 0.2,
				width: 0.5,
				height: 0.28,
				is_auto: true
			}),
			frame({ id: -3, x: 0, y: 0, width: 1, height: 1 })
		],
		focalPoints: [
			{ id: -2, frame_id: undefined, x: 0.4, y: 0.45, label: 'Eyes', required: true },
			{ id: -4, frame_id: -1, x: 0.8, y: 0.1, label: undefined, required: false }
		]
	};

	test('round-trips a document written without a database', () => {
		const restored = docFromSidecar(JSON.parse(sidecarJson(DOC, '/photos/beach.jpg')));
		expect(restored).toEqual(DOC);
	});

	test('keeps the local negative ids so a resumed session keeps minting below them', () => {
		const restored = docFromSidecar(JSON.parse(sidecarJson(DOC)));
		expect(restored.frames.map((f) => f.id)).toEqual([-1, -3]);
		expect(restored.focalPoints.map((p) => p.id)).toEqual([-2, -4]);
	});

	test('carries a fit section for destination frames only', () => {
		const raw = JSON.parse(sidecarJson(DOC, '/photos/beach.jpg')) as { fit: { slug: string }[] };
		expect(raw.fit.map((f) => f.slug)).toEqual(['desktop']);
	});
});
