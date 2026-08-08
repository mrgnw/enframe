import type { FocalPoint, Frame } from './db';

export const HISTORY_CAP = 100;
export const HISTORY_DEBOUNCE_MS = 500;

export interface HistorySnapshot {
	frames: Frame[];
	focalPoints: FocalPoint[];
}

function frameRecord(frame: Frame): Frame {
	return {
		id: frame.id,
		photo_id: frame.photo_id,
		label: frame.label,
		destination: frame.destination ?? null,
		x: frame.x,
		y: frame.y,
		width: frame.width,
		height: frame.height,
		rotation: frame.rotation ?? 0,
		is_default: !!frame.is_default,
		is_auto: !!frame.is_auto,
		created_at: frame.created_at
	};
}

function pointRecord(point: FocalPoint): FocalPoint {
	return {
		id: point.id,
		photo_id: point.photo_id,
		frame_id: point.frame_id ?? undefined,
		x: point.x,
		y: point.y,
		label: point.label,
		required: !!point.required,
		created_at: point.created_at
	};
}

export function serializeSnapshot(frames: Frame[], focalPoints: FocalPoint[]): string {
	return JSON.stringify({
		frames: frames.map(frameRecord),
		focalPoints: focalPoints.map(pointRecord)
	});
}

export function parseSnapshot(text: string): HistorySnapshot {
	const raw = JSON.parse(text) as Partial<HistorySnapshot>;
	return {
		frames: (raw.frames ?? []).map(frameRecord),
		focalPoints: (raw.focalPoints ?? []).map(pointRecord)
	};
}

export function clampPos(seqs: number[], pos: number): number {
	if (seqs.length === 0) return 0;
	if (seqs.includes(pos)) return pos;
	return Math.max(...seqs);
}

export function undoSeq(seqs: number[], pos: number): number | null {
	const earlier = seqs.filter((s) => s < pos);
	return earlier.length === 0 ? null : Math.max(...earlier);
}

export function redoSeq(seqs: number[], pos: number): number | null {
	const later = seqs.filter((s) => s > pos);
	return later.length === 0 ? null : Math.min(...later);
}

export function planAppend(seqs: number[], pos: number, cap: number = HISTORY_CAP): { drop: number[]; seq: number } {
	const kept = seqs.filter((s) => s <= pos).sort((a, b) => a - b);
	const drop = seqs.filter((s) => s > pos);
	const overflow = kept.length + 1 - cap;
	if (overflow > 0) drop.push(...kept.slice(0, overflow));
	const highest = seqs.length === 0 ? 0 : Math.max(...seqs);
	return { drop, seq: highest + 1 };
}

export function remapId(id: number | null, idMap: ReadonlyMap<number, number>): number | null {
	if (id === null) return null;
	return idMap.get(id) ?? null;
}

export function remapSelection(selection: Iterable<number>, idMap: ReadonlyMap<number, number>): Set<number> {
	const next = new Set<number>();
	for (const id of selection) {
		const mapped = idMap.get(id);
		if (mapped !== undefined) next.add(mapped);
	}
	return next;
}
