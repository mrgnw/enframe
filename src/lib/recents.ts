import type { CropDoc } from './framer/types';

export const RECENTS_KEY = 'enframe.recents';
export const RECENTS_CAP = 20;

interface RecentBase {
	name: string;
	width: number;
	height: number;
	openedAt: number;
}

export interface LibraryRecent extends RecentBase {
	kind: 'library';
	photoId: number;
}

export interface StandaloneRecent extends RecentBase {
	kind: 'standalone';
	doc: CropDoc;
}

export type Recent = LibraryRecent | StandaloneRecent;

export function recentId(entry: Recent): string {
	return entry.kind === 'library'
		? `library:${entry.photoId}`
		: `standalone:${entry.name}:${entry.width}x${entry.height}`;
}

function isRecent(value: unknown): value is Recent {
	if (typeof value !== 'object' || value === null) return false;
	const entry = value as Record<string, unknown>;
	if (typeof entry.name !== 'string') return false;
	if (!Number.isFinite(entry.width) || !Number.isFinite(entry.height)) return false;
	if (!Number.isFinite(entry.openedAt)) return false;
	if (entry.kind === 'library') return Number.isFinite(entry.photoId);
	return entry.kind === 'standalone' && typeof entry.doc === 'object' && entry.doc !== null;
}

// A recents list is a convenience, never a source of truth: anything unreadable is
// simply not recent anymore.
export function parseRecents(raw: string | null): Recent[] {
	if (!raw) return [];
	let parsed: unknown;
	try {
		parsed = JSON.parse(raw);
	} catch {
		return [];
	}
	if (!Array.isArray(parsed)) return [];
	return parsed.filter(isRecent).slice(0, RECENTS_CAP);
}

export function withRecent(list: Recent[], entry: Recent, cap = RECENTS_CAP): Recent[] {
	const id = recentId(entry);
	return [entry, ...list.filter((other) => recentId(other) !== id)].slice(0, cap);
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export function relativeTime(openedAt: number, now: number): string {
	const elapsed = now - openedAt;
	if (elapsed < MINUTE) return 'just now';
	if (elapsed < HOUR) return `${Math.floor(elapsed / MINUTE)}m ago`;
	if (elapsed < DAY) return `${Math.floor(elapsed / HOUR)}h ago`;
	if (elapsed < 7 * DAY) return `${Math.floor(elapsed / DAY)}d ago`;
	return new Date(openedAt).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

export function loadRecents(): Recent[] {
	if (typeof localStorage === 'undefined') return [];
	return parseRecents(localStorage.getItem(RECENTS_KEY));
}

// ponytail: the frames come back, the file does not — the user re-picks it. Storing the
// FileSystemHandle in IndexedDB would reopen it directly, and is the upgrade path here.
export function rememberRecent(entry: Recent): Recent[] {
	const next = withRecent(loadRecents(), entry);
	if (typeof localStorage === 'undefined') return next;
	try {
		localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
	} catch (error) {
		console.error('Failed to store recents:', error);
	}
	return next;
}
