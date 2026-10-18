export type Direction = 'columns' | 'rows';
export type Energy = 'forward' | 'gradient';
export type PointKind = 'protect' | 'remove';

// Points live in original-photo coordinates, 0–1 on each axis. `size` is the disc's radius as a
// fraction of the photo's short side; `strength` 0–10 is how hard seams avoid a protect disc.
export interface CarvePoint {
	kind: PointKind;
	x: number;
	y: number;
	size: number;
	strength: number;
}

export interface CarveSettings {
	direction: Direction;
	/** The target size as a share of the original along the carved axis, 0.4–1.4. */
	size: number;
	energy: Energy;
	blur: number;
	straight: number;
	lean: number;
	from: number;
	to: number;
	working: number;
}

export const DEFAULT_SETTINGS: CarveSettings = {
	direction: 'columns',
	size: 0.75,
	energy: 'forward',
	blur: 0,
	straight: 0,
	lean: 0,
	from: 0,
	to: 1,
	working: 1024
};

/** What `photos.carve` stores: the settings and points a version was made with. */
export interface CarveRecord {
	settings: CarveSettings;
	points: CarvePoint[];
}

export const MAX_REMOVED = 0.6;
export const MIN_POINT_SIZE = 0.01;
export const MAX_POINT_SIZE = 0.4;
