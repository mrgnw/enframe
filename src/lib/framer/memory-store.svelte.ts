import { parseSnapshot } from '../history';
import type { FramerStore, RestoredHistory } from './session.svelte';
import { type CropDoc, type FocalPoint, type Frame, nextLocalId } from './types';

// A store for a host with nowhere to put anything. The session already owns the live
// frames and points, so the six CRUD ops only have to hand out ids that will not
// collide; the history map is the part that earns its keep, since without a store the
// session can never journal a snapshot and undo stays dead.
//
// Restoring keeps the ids the snapshot recorded, so the id map handed back is the
// identity — the framer still uses it to drop selections whose frame is gone.
export class MemoryStore implements FramerStore {
	#nextId: number;
	#snapshots = new Map<number, string>();

	constructor(doc: CropDoc) {
		this.#nextId = nextLocalId(doc);
	}

	get size(): number {
		return this.#snapshots.size;
	}

	#mint(): number {
		const id = this.#nextId;
		this.#nextId -= 1;
		return id;
	}

	async addFrame(_frame: Omit<Frame, 'id'>): Promise<number> {
		return this.#mint();
	}

	async updateFrame(_id: number, _patch: Partial<Frame>): Promise<void> {}

	async deleteFrame(_id: number): Promise<void> {}

	async addPoint(_point: Omit<FocalPoint, 'id'>): Promise<number> {
		return this.#mint();
	}

	async updatePoint(_id: number, _patch: Partial<FocalPoint>): Promise<void> {}

	async deletePoint(_id: number): Promise<void> {}

	async saveHistory(seq: number, drop: number[], snapshot: string): Promise<void> {
		for (const stale of drop) this.#snapshots.delete(stale);
		this.#snapshots.set(seq, snapshot);
	}

	async loadHistory(seq: number): Promise<RestoredHistory | null> {
		const stored = this.#snapshots.get(seq);
		if (stored === undefined) return null;
		const { frames, focalPoints } = parseSnapshot(stored);
		return { frames, focalPoints, frameIds: new Map(frames.map((f) => [f.id, f.id])) };
	}
}
