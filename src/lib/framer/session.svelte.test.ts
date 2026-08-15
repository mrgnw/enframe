import { describe, expect, test } from 'vitest';
import { FramerSession } from './session.svelte';
import { CROP_DOC_VERSION, type CropDoc, type FocalPoint, type Frame } from './types';

function frame(id: number): Frame {
	return {
		id,
		label: undefined,
		destination: null,
		x: 0.1,
		y: 0.1,
		width: 0.2,
		height: 0.2,
		rotation: 0,
		is_default: false,
		is_auto: false
	};
}

function point(id: number): FocalPoint {
	return { id, frame_id: undefined, x: 0.5, y: 0.5, label: `Point ${id}`, required: true };
}

function doc(frames: Frame[], focalPoints: FocalPoint[]): CropDoc {
	return {
		version: CROP_DOC_VERSION,
		image: { name: 'test.jpg', width: 4000, height: 3000 },
		frames,
		focalPoints
	};
}

function session(frameCount: number, pointCount: number) {
	const frames = Array.from({ length: frameCount }, (_, i) => frame(i + 1));
	const points = Array.from({ length: pointCount }, (_, i) => point(i + 1));
	return new FramerSession({ doc: doc(frames, points) });
}

const ids = (set: Set<number>) => [...set].sort((a, b) => a - b);

describe('point range selection', () => {
	test('shift-click extends from the anchor', () => {
		const s = session(0, 6);
		s.selectPoint(1);
		s.selectPoint(5, 'range');
		expect(ids(s.pointSelection)).toEqual([1, 2, 3, 4, 5]);
	});

	// The anchor stays put across range extensions, so clicking nearer shrinks the range.
	test('a second shift-click shrinks the range instead of growing it', () => {
		const s = session(0, 6);
		s.selectPoint(1);
		s.selectPoint(5, 'range');
		s.selectPoint(3, 'range');
		expect(ids(s.pointSelection)).toEqual([1, 2, 3]);
	});

	test('extends backwards when the clicked point precedes the anchor', () => {
		const s = session(0, 6);
		s.selectPoint(4);
		s.selectPoint(2, 'range');
		expect(ids(s.pointSelection)).toEqual([2, 3, 4]);
	});

	test('a plain click moves the anchor', () => {
		const s = session(0, 6);
		s.selectPoint(1);
		s.selectPoint(5, 'range');
		s.selectPoint(3);
		s.selectPoint(5, 'range');
		expect(ids(s.pointSelection)).toEqual([3, 4, 5]);
	});

	// Deleting the anchor must not strand the next shift-click into replacing everything.
	test('range survives deletion of the anchor', async () => {
		const s = session(0, 8);
		s.selectPoint(4);
		s.selectPoint(2, 'range');
		expect(ids(s.pointSelection)).toEqual([2, 3, 4]);

		await s.deletePoints([4]);
		expect(ids(s.pointSelection)).toEqual([2, 3]);

		// Points are now 1,2,3,5,6,7,8; the origin falls back to 3, the last one still selected.
		s.selectPoint(6, 'range');
		expect(ids(s.pointSelection)).toEqual([3, 5, 6]);
	});

	test('the first shift-click with no anchor selects one point', () => {
		const s = session(0, 4);
		s.selectPoint(3, 'range');
		expect(ids(s.pointSelection)).toEqual([3]);
	});
});

describe('point toggle selection', () => {
	test('cmd-click adds and removes', () => {
		const s = session(0, 4);
		s.selectPoint(1);
		s.selectPoint(3, 'toggle');
		expect(ids(s.pointSelection)).toEqual([1, 3]);

		s.selectPoint(1, 'toggle');
		expect(ids(s.pointSelection)).toEqual([3]);
	});

	test('a plain click replaces the whole selection', () => {
		const s = session(0, 4);
		s.selectPoint(1);
		s.selectPoint(2, 'toggle');
		s.selectPoint(4);
		expect(ids(s.pointSelection)).toEqual([4]);
	});
});

describe('frame and point selections stay consistent', () => {
	test('selecting a frame clears the point selection', () => {
		const s = session(2, 3);
		s.selectPoint(1);
		s.selectPoint(3, 'range');
		s.selectFrame(1);
		expect(ids(s.pointSelection)).toEqual([]);
		expect(s.selectedPointId).toBeNull();
	});

	test('select-all-frames clears the point selection', () => {
		const s = session(2, 3);
		s.selectPoint(2);
		s.selectAllFrames();
		expect(ids(s.pointSelection)).toEqual([]);
		expect(s.selectedPointId).toBeNull();
	});

	test('deselect clears both', () => {
		const s = session(2, 3);
		s.selectPoint(1);
		s.selectPoint(3, 'range');
		s.deselect();
		expect(ids(s.pointSelection)).toEqual([]);
		expect(ids(s.selection)).toEqual([]);
	});

	// Picking a point is not a statement about what to export: the frame selection is the
	// export set, and losing it silently widens an export to every frame.
	test('selecting a point keeps the frame selection intact', () => {
		const s = session(3, 2);
		s.selectFrame(1);
		s.selectFrame(2, 'toggle');
		s.selectPoint(1);
		expect(ids(s.selection)).toEqual([1, 2]);
		expect(s.framesToExport.map((f) => f.id)).toEqual([1, 2]);
	});

	test('stepping through points keeps the frame selection intact', () => {
		const s = session(3, 2);
		s.selectFrame(1);
		s.selectNextPoint();
		s.selectNextPoint();
		expect(ids(s.selection)).toEqual([1]);
		expect(s.framesToExport.map((f) => f.id)).toEqual([1]);
	});
});

describe('point navigation', () => {
	test('wraps forwards and backwards', () => {
		const s = session(0, 3);
		s.selectNextPoint();
		expect(s.selectedPointId).toBe(1);
		s.selectNextPoint();
		s.selectNextPoint();
		expect(s.selectedPointId).toBe(3);
		s.selectNextPoint();
		expect(s.selectedPointId).toBe(1);
		s.selectPrevPoint();
		expect(s.selectedPointId).toBe(3);
	});

	test('does nothing without points', () => {
		const s = session(2, 0);
		s.selectNextPoint();
		expect(s.selectedPointId).toBeNull();
	});

	test('navigation replaces the point selection', () => {
		const s = session(0, 4);
		s.selectPoint(1);
		s.selectPoint(3, 'range');
		s.selectNextPoint();
		expect(ids(s.pointSelection)).toEqual([4]);
	});
});

describe('deleting points', () => {
	test('deletes the whole selection and reports the count', async () => {
		const s = session(0, 5);
		s.selectPoint(2);
		s.selectPoint(4, 'range');
		expect(await s.deleteSelectedPoints()).toBe(3);
		expect(s.focalPoints.map((p) => p.id)).toEqual([1, 5]);
		expect(ids(s.pointSelection)).toEqual([]);
	});

	test('deletes the single point reached by navigation', async () => {
		const s = session(0, 3);
		s.selectNextPoint();
		expect(await s.deleteSelectedPoints()).toBe(1);
		expect(s.focalPoints.map((p) => p.id)).toEqual([2, 3]);
	});

	test('deletes nothing when no point is selected', async () => {
		const s = session(0, 3);
		expect(await s.deleteSelectedPoints()).toBe(0);
		expect(s.focalPoints).toHaveLength(3);
	});

	test('ignores ids that are not present', async () => {
		const s = session(0, 2);
		expect(await s.deletePoints([99])).toBe(0);
		expect(s.focalPoints).toHaveLength(2);
	});
});
