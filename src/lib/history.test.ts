import { describe, expect, test } from 'bun:test';
import type { FocalPoint, Frame } from './framer/types';
import {
	clampPos,
	HISTORY_CAP,
	parseSnapshot,
	planAppend,
	redoSeq,
	remapId,
	remapSelection,
	serializeSnapshot,
	undoSeq
} from './history';

const FRAMES: Frame[] = [
	{
		id: 7,
		label: 'Instagram Portrait',
		destination: 'instagram-portrait',
		x: 0.1,
		y: 0.2,
		width: 0.5,
		height: 0.625,
		rotation: 1.5,
		is_default: false,
		is_auto: true
	},
	{
		id: 9,
		label: undefined,
		destination: null,
		x: 0,
		y: 0,
		width: 1,
		height: 1,
		rotation: 0,
		is_default: true,
		is_auto: false
	}
];

const POINTS: FocalPoint[] = [
	{
		id: 1,
		frame_id: 7,
		x: 0.4,
		y: 0.45,
		label: 'Eyes',
		required: true
	},
	{
		id: 2,
		frame_id: undefined,
		x: 0.8,
		y: 0.1,
		label: undefined,
		required: false
	}
];

describe('snapshot round-trip', () => {
	test('preserves every frame and focal point field', () => {
		const restored = parseSnapshot(serializeSnapshot(FRAMES, POINTS));
		expect(restored.frames).toEqual(FRAMES);
		expect(restored.focalPoints).toEqual(POINTS);
	});

	test('is stable across repeated round-trips', () => {
		const once = serializeSnapshot(FRAMES, POINTS);
		const twice = serializeSnapshot(parseSnapshot(once).frames, parseSnapshot(once).focalPoints);
		expect(twice).toBe(once);
	});

	test('normalises sqlite integer booleans', () => {
		const rows = [{ ...FRAMES[0], is_auto: 1 as unknown as boolean, is_default: 0 as unknown as boolean }];
		const restored = parseSnapshot(serializeSnapshot(rows, [{ ...POINTS[0], required: 0 as unknown as boolean }]));
		expect(restored.frames[0].is_auto).toBe(true);
		expect(restored.frames[0].is_default).toBe(false);
		expect(restored.focalPoints[0].required).toBe(false);
	});

	test('reads snapshots written before the document split, dropping row columns', () => {
		const legacy = JSON.stringify({
			frames: [{ ...FRAMES[0], photo_id: 3, created_at: '2026-01-01 00:00:00' }],
			focalPoints: [{ ...POINTS[0], photo_id: 3, created_at: '2026-01-01 00:00:00' }]
		});
		const restored = parseSnapshot(legacy);
		expect(restored.frames).toEqual([FRAMES[0]]);
		expect(restored.focalPoints).toEqual([POINTS[0]]);
	});

	test('handles an empty state', () => {
		expect(parseSnapshot(serializeSnapshot([], []))).toEqual({ frames: [], focalPoints: [] });
	});
});

describe('undo pointer math', () => {
	test('undo walks back, redo walks forward', () => {
		const seqs = [1, 2, 3, 4];
		expect(undoSeq(seqs, 4)).toBe(3);
		expect(undoSeq(seqs, 2)).toBe(1);
		expect(undoSeq(seqs, 1)).toBeNull();
		expect(redoSeq(seqs, 1)).toBe(2);
		expect(redoSeq(seqs, 4)).toBeNull();
	});

	test('skips gaps left by evicted entries', () => {
		const seqs = [5, 9, 12];
		expect(undoSeq(seqs, 12)).toBe(9);
		expect(undoSeq(seqs, 5)).toBeNull();
		expect(redoSeq(seqs, 5)).toBe(9);
	});

	test('empty history cannot undo or redo', () => {
		expect(undoSeq([], 0)).toBeNull();
		expect(redoSeq([], 0)).toBeNull();
	});
});

describe('clampPos', () => {
	test('keeps a valid pointer', () => {
		expect(clampPos([1, 2, 3], 2)).toBe(2);
	});

	test('falls back to the newest entry when the stored pointer is stale', () => {
		expect(clampPos([4, 5, 6], 2)).toBe(6);
		expect(clampPos([4, 5, 6], 0)).toBe(6);
	});

	test('is zero when there is no history', () => {
		expect(clampPos([], 7)).toBe(0);
	});
});

describe('planAppend', () => {
	test('appends after the newest entry', () => {
		expect(planAppend([1, 2, 3], 3)).toEqual({ drop: [], seq: 4 });
	});

	test('starts at seq 1', () => {
		expect(planAppend([], 0)).toEqual({ drop: [], seq: 1 });
	});

	test('drops the redo branch when editing after an undo', () => {
		expect(planAppend([1, 2, 3, 4], 2)).toEqual({ drop: [3, 4], seq: 5 });
	});

	test('never reuses a seq that is being dropped', () => {
		const { drop, seq } = planAppend([1, 2, 3, 4], 1);
		expect(drop).not.toContain(seq);
		expect(seq).toBeGreaterThan(Math.max(...drop));
	});

	test('evicts the oldest entries beyond the cap', () => {
		const seqs = Array.from({ length: HISTORY_CAP }, (_, i) => i + 1);
		const { drop, seq } = planAppend(seqs, HISTORY_CAP);
		expect(drop).toEqual([1]);
		expect(seq).toBe(HISTORY_CAP + 1);
	});

	test('evicts enough to stay at the cap when over it', () => {
		const seqs = Array.from({ length: HISTORY_CAP + 3 }, (_, i) => i + 1);
		const { drop } = planAppend(seqs, HISTORY_CAP + 3);
		expect(drop).toEqual([1, 2, 3, 4]);
		expect(seqs.length - drop.length + 1).toBe(HISTORY_CAP);
	});

	test('holds the cap across a long run of edits', () => {
		let seqs: number[] = [];
		let pos = 0;
		for (let i = 0; i < HISTORY_CAP * 3; i++) {
			const { drop, seq } = planAppend(seqs, pos, 5);
			seqs = seqs.filter((s) => !drop.includes(s)).concat(seq);
			pos = seq;
			expect(seqs.length).toBeLessThanOrEqual(5);
		}
		expect(seqs).toEqual([296, 297, 298, 299, 300]);
	});
});

describe('id remapping', () => {
	const idMap = new Map([
		[7, 21],
		[9, 22]
	]);

	test('remaps a surviving selection', () => {
		expect(remapId(7, idMap)).toBe(21);
		expect(remapSelection([7, 9], idMap)).toEqual(new Set([21, 22]));
	});

	test('drops ids the restored snapshot does not contain', () => {
		expect(remapId(99, idMap)).toBeNull();
		expect(remapSelection([7, 99], idMap)).toEqual(new Set([21]));
	});

	test('leaves a null selection null', () => {
		expect(remapId(null, idMap)).toBeNull();
		expect(remapSelection([], idMap)).toEqual(new Set());
	});
});
