// The marking screen's data: the document it edits and the sidecar it draws (docs/ui-plan.md, "Data model").
// Points on screen are normalized 0–1 to the photo; the document sends the CLI pixels.

export type Point = [number, number];
export type Source = 'tracked' | 'mark' | 'soft' | 'lost' | 'skipped';

/** The CLI's input, saved beside the folder as `<folder>.pin.json`; pixels of the oriented photo. */
export interface Marks {
	points: Point[];
	at?: Record<string, { points: Point[]; zoom?: number }>;
	/** Soft marks: placed there, but the tracker carries on from the last photo before them (`--near`). */
	near?: Record<string, { points: Point[]; zoom?: number }>;
	skip?: string[];
	order?: 'time' | 'name';
}

/** One row of the sidecar, as /api/track returns it, with the size and note it adds from the run's log. */
export interface SidecarPhoto {
	file: string;
	x: number;
	y: number;
	w: number;
	h: number;
	scale: number;
	source: Source;
	points: Point[];
	width?: number;
	height?: number;
	note?: string;
}

export interface Sidecar {
	run: string;
	clip: string | null;
	photos: SidecarPhoto[];
}

/** A photo as the screen holds it: the sidecar row plus what the mockup's photos.js carried. */
export interface Photo {
	file: string;
	/** The file name without its extension, for labels. */
	name: string;
	/** Oriented pixel size. */
	w: number;
	h: number;
	scale: number;
	/** The sidecar's last confirmed scale, unlike `scale` which a local mark sets ahead of the retrack that confirms it. */
	refScale: number;
	source: Source;
	/** Normalized 0–1 to this photo. */
	pts: Point[];
	/** Agreeing features for tracked and lost photos, null for marks and skips. */
	feats: number | null;
}

export const ZOOMS = [1, 1.5, 2, 2.5, 3, 5];
export const TRAIL_MIN = 5;
export const TRAIL_DEFAULT = 6;
export const TRAIL_MAX = 10;
export const ADVANCE_MS = 450;
export const TOAST_MS = 6000;
export const HISTORY_MAX = 100;
export const HIT_PX = 12;
export const PATCH_PX = 600;
export type TrailMode = 'fade' | 'steps' | 'flow';

export const centre = (p: { pts: Point[] }): Point => [
	p.pts.reduce((s, q) => s + q[0], 0) / p.pts.length,
	p.pts.reduce((s, q) => s + q[1], 0) / p.pts.length
];

export const seen = (p: Photo) => p.source === 'tracked' || p.source === 'mark';

export const clamp = (x: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, x));

/** Whether every point lands inside the photo, matching the CLI's own `0 <= p < size` check. */
export const inBounds = (pts: Point[]) => pts.every(([x, y]) => x >= 0 && x < 1 && y >= 0 && y < 1);

/** A sidecar row as a `Photo`; sizes fall back to the sidecar's crop ratio until the log line is known. */
export function fromSidecar(row: SidecarPhoto): Photo {
	const feats = row.note ? /(\d+) features/.exec(row.note) : null;
	return {
		file: row.file,
		name: row.file.replace(/\.[^.]+$/, ''),
		w: row.width ?? 0,
		h: row.height ?? 0,
		scale: row.scale,
		refScale: row.scale,
		source: row.source,
		pts: row.points.map(([x, y]) => [x, y]),
		feats: feats ? Number(feats[1]) : null
	};
}
