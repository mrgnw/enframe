import { isLegacyAutoLabel } from '../composition';

export const CROP_DOC_VERSION = 1;

export type ExportFormat = 'avif' | 'jpg' | 'png';

// Identity without a database: ids are unique within one document, nothing more.
// A library host fills them with SQLite rowids, which are always positive. The framer
// mints negative ids (see nextLocalId) for items it creates; a host that persists an
// item reports the real id back and the framer remaps selections onto it. A host with
// no storage keeps the negative ids, which round-trip through the sidecar unchanged.
export interface Frame {
	id: number;
	label?: string;
	destination?: string | null;
	x: number;
	y: number;
	width: number;
	height: number;
	rotation: number;
	is_default: boolean;
	is_auto: boolean;
}

export interface FocalPoint {
	id: number;
	frame_id?: number;
	x: number;
	y: number;
	label?: string;
	required: boolean;
}

export interface DocImage {
	name: string;
	width: number;
	height: number;
}

export interface CropDoc {
	version: number;
	image: DocImage;
	frames: Frame[];
	focalPoints: FocalPoint[];
}

export interface SidecarFrame {
	id: number;
	label?: string;
	destination: string | null;
	is_auto: boolean;
	x: number;
	y: number;
	width: number;
	height: number;
	rotation: number;
	is_default: boolean;
}

export interface SidecarPoint {
	id: number;
	frame_id?: number;
	label?: string;
	x: number;
	y: number;
	required: boolean;
}

export interface Sidecar {
	version: number;
	photo: { title?: string; width?: number; height?: number; file_path?: string };
	frames: SidecarFrame[];
	focal_points: SidecarPoint[];
}

export function emptyDoc(): CropDoc {
	return { version: CROP_DOC_VERSION, image: { name: '', width: 0, height: 0 }, frames: [], focalPoints: [] };
}

export function frameRecord(frame: Frame): Frame {
	return {
		id: frame.id,
		label: frame.label,
		destination: frame.destination ?? null,
		x: frame.x,
		y: frame.y,
		width: frame.width,
		height: frame.height,
		rotation: frame.rotation ?? 0,
		is_default: !!frame.is_default,
		is_auto: !!frame.is_auto
	};
}

export function pointRecord(point: FocalPoint): FocalPoint {
	return {
		id: point.id,
		frame_id: point.frame_id ?? undefined,
		x: point.x,
		y: point.y,
		label: point.label,
		required: !!point.required
	};
}

export function docToSidecar(doc: CropDoc, filePath?: string): Sidecar {
	return {
		version: CROP_DOC_VERSION,
		photo: {
			title: doc.image.name,
			width: doc.image.width,
			height: doc.image.height,
			file_path: filePath
		},
		frames: doc.frames.map((f) => ({ ...frameRecord(f), destination: f.destination ?? null })),
		focal_points: doc.focalPoints.map((p) => ({
			id: p.id,
			frame_id: p.frame_id,
			label: p.label,
			x: p.x,
			y: p.y,
			required: !!p.required
		}))
	};
}

// Tolerant of every sidecar written before the version field existed.
export function docFromSidecar(raw: Partial<Sidecar>): CropDoc {
	const photo = raw.photo ?? {};
	return {
		version: CROP_DOC_VERSION,
		image: { name: photo.title ?? '', width: photo.width ?? 0, height: photo.height ?? 0 },
		frames: (raw.frames ?? []).map((f) => ({
			id: f.id,
			label: f.label,
			destination: f.destination ?? null,
			x: f.x,
			y: f.y,
			width: f.width,
			height: f.height,
			rotation: f.rotation ?? 0,
			is_default: !!f.is_default,
			is_auto: !!f.is_auto || isLegacyAutoLabel(f.label)
		})),
		focalPoints: (raw.focal_points ?? []).map(pointRecord)
	};
}

// Step through an item list, wrapping at both ends. An unknown current id lands on
// the first item going forwards and the second-to-last going backwards, which is what
// falls out of the wrap and keeps `,`/`.` usable after an item disappears.
export function cycleFrameId(items: { id: number }[], current: number | null, delta: 1 | -1): number | null {
	if (items.length === 0) return null;
	if (current === null) return delta > 0 ? items[0].id : items[items.length - 1].id;
	const index = items.findIndex((f) => f.id === current);
	return items[(index + delta + items.length) % items.length].id;
}

// Frames whose box contains the normalised point, topmost first: later frames paint over
// earlier ones, so render order reverses into hit order. Feed the result straight to
// cycleFrameId to step down through a stack of overlapping frames at one spot.
// ponytail: axis-aligned test, rotation ignored — frames only rotate by 0 today.
export function framesAtPoint(frames: Frame[], x: number, y: number): Frame[] {
	const hits: Frame[] = [];
	for (let i = frames.length - 1; i >= 0; i--) {
		const frame = frames[i];
		const inside = x >= frame.x && x <= frame.x + frame.width && y >= frame.y && y <= frame.y + frame.height;
		if (inside) hits.push(frame);
	}
	return hits;
}

export type SelectMode = 'replace' | 'toggle';

// Points add 'range': in the panel list, shift-click extends from the anchor.
export type PointSelectMode = SelectMode | 'range';

// The anchor is the fixed origin a range extends from; it moves only on a plain or
// toggling click, never on a range extension, so shift-clicking a nearer point shrinks
// the range instead of growing it. The focus is the point the UI marks as active.
export interface PointSelection {
	selection: Set<number>;
	anchor: number | null;
	focus: number | null;
}

// Deleting the anchor must not strand a range extension: fall back to the last still-live
// selected point so shift-click keeps extending instead of collapsing to one point.
function rangeOrigin(current: PointSelection, points: { id: number }[]): number | null {
	if (current.anchor !== null && points.some((p) => p.id === current.anchor)) return current.anchor;
	for (let i = points.length - 1; i >= 0; i--) {
		if (current.selection.has(points[i].id)) return points[i].id;
	}
	return null;
}

export function nextPointSelection(
	current: PointSelection,
	points: { id: number }[],
	pointId: number,
	mode: PointSelectMode
): PointSelection {
	if (mode === 'range') {
		const origin = rangeOrigin(current, points);
		const from = points.findIndex((p) => p.id === origin);
		const to = points.findIndex((p) => p.id === pointId);
		if (from !== -1 && to !== -1) {
			const [lo, hi] = from < to ? [from, to] : [to, from];
			const selection = new Set(points.slice(lo, hi + 1).map((p) => p.id));
			return { selection, anchor: points[from].id, focus: pointId };
		}
	}
	const next = nextSelection(current.selection, pointId, mode === 'toggle' ? 'toggle' : 'replace');
	return { selection: next.selection, anchor: pointId, focus: next.anchor };
}

// One selection drives editing, export and bulk actions. A plain click replaces it, shift
// or cmd toggles a frame in or out. The anchor is the frame the canvas keeps its handles
// on: the clicked frame while it stays selected, otherwise the last one still standing.
export function nextSelection(
	selection: ReadonlySet<number>,
	frameId: number,
	mode: SelectMode
): { selection: Set<number>; anchor: number | null } {
	if (mode === 'replace') return { selection: new Set([frameId]), anchor: frameId };
	const next = new Set(selection);
	if (next.has(frameId)) {
		next.delete(frameId);
		return { selection: next, anchor: [...next].at(-1) ?? null };
	}
	next.add(frameId);
	return { selection: next, anchor: frameId };
}

export function nextLocalId(doc: CropDoc): number {
	let lowest = 0;
	for (const frame of doc.frames) lowest = Math.min(lowest, frame.id);
	for (const point of doc.focalPoints) lowest = Math.min(lowest, point.id);
	return lowest - 1;
}
