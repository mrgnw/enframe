import { describe, expect, test } from 'bun:test';
import { HISTORY_CAP, planAppend, serializeSnapshot } from '../history';
import { MemoryStore } from './memory-store.svelte';
import { type CropDoc, CROP_DOC_VERSION, emptyDoc, type FocalPoint, type Frame } from './types';

function frame(id: number): Frame {
	return {
		id,
		label: 'Crop',
		destination: null,
		x: 0.1,
		y: 0.2,
		width: 0.3,
		height: 0.4,
		rotation: 0,
		is_default: false,
		is_auto: false
	};
}

function point(id: number): FocalPoint {
	return { id, frame_id: undefined, x: 0.5, y: 0.5, label: 'Point 1', required: true };
}

const DOC: CropDoc = {
	version: CROP_DOC_VERSION,
	image: { name: 'beach.jpg', width: 100, height: 50 },
	frames: [frame(-1)],
	focalPoints: [point(-2)]
};

describe('id minting', () => {
	test('starts below every id already in the document', async () => {
		const store = new MemoryStore(DOC);
		expect(await store.addFrame(frame(0))).toBe(-3);
		expect(await store.addPoint(point(0))).toBe(-4);
		expect(await store.addFrame(frame(0))).toBe(-5);
	});

	test('starts at -1 for an empty document', async () => {
		expect(await new MemoryStore(emptyDoc()).addFrame(frame(0))).toBe(-1);
	});
});

describe('history', () => {
	test('restores the frames and points a snapshot recorded', async () => {
		const store = new MemoryStore(DOC);
		await store.saveHistory(1, [], serializeSnapshot(DOC.frames, DOC.focalPoints));
		const restored = await store.loadHistory(1);
		expect(restored?.frames).toEqual(DOC.frames);
		expect(restored?.focalPoints).toEqual(DOC.focalPoints);
	});

	test('maps restored ids onto themselves and forgets ids the snapshot lost', async () => {
		const store = new MemoryStore(DOC);
		await store.saveHistory(1, [], serializeSnapshot([frame(-1)], []));
		const restored = await store.loadHistory(1);
		expect(restored?.frameIds.get(-1)).toBe(-1);
		expect(restored?.frameIds.get(-9)).toBeUndefined();
	});

	test('an unknown seq restores nothing', async () => {
		expect(await new MemoryStore(DOC).loadHistory(4)).toBeNull();
	});

	test('drops the seqs the caller superseded', async () => {
		const store = new MemoryStore(DOC);
		await store.saveHistory(1, [], serializeSnapshot([frame(-1)], []));
		await store.saveHistory(2, [], serializeSnapshot([frame(-3)], []));
		await store.saveHistory(3, [2], serializeSnapshot([frame(-5)], []));
		expect(await store.loadHistory(2)).toBeNull();
		expect((await store.loadHistory(1))?.frames[0].id).toBe(-1);
		expect((await store.loadHistory(3))?.frames[0].id).toBe(-5);
	});

	test('stays capped while the session keeps journalling edits', async () => {
		const store = new MemoryStore(DOC);
		let seqs: number[] = [];
		let pos = 0;
		for (let i = 0; i < HISTORY_CAP + 50; i++) {
			const { drop, seq } = planAppend(seqs, pos);
			await store.saveHistory(seq, drop, serializeSnapshot([frame(-1 - i)], []));
			seqs = seqs.filter((s) => !drop.includes(s)).concat(seq);
			pos = seq;
		}
		expect(store.size).toBe(HISTORY_CAP);
		expect(seqs).toHaveLength(HISTORY_CAP);
		expect(await store.loadHistory(seqs[0])).not.toBeNull();
		expect(await store.loadHistory(seqs[0] - 1)).toBeNull();
	});
});
