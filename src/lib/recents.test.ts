import { describe, expect, test } from 'bun:test';
import { emptyDoc } from './framer/types';
import { type Recent, RECENTS_CAP, parseRecents, recentId, relativeTime, withRecent } from './recents';

function library(photoId: number, openedAt = 1000): Recent {
	return { kind: 'library', photoId, name: `photo ${photoId}`, width: 6000, height: 4000, openedAt };
}

function standalone(name: string, width = 4000, height = 3000, openedAt = 1000): Recent {
	return { kind: 'standalone', name, width, height, openedAt, doc: emptyDoc() };
}

describe('identity', () => {
	test('library entries are the same photo regardless of name or size', () => {
		expect(recentId(library(7))).toBe(recentId({ ...library(7), name: 'renamed', width: 10 }));
	});

	test('standalone entries are keyed by name and dimensions', () => {
		expect(recentId(standalone('a.jpg'))).toBe(recentId(standalone('a.jpg')));
		expect(recentId(standalone('a.jpg'))).not.toBe(recentId(standalone('a.jpg', 4001)));
		expect(recentId(standalone('a.jpg'))).not.toBe(recentId(standalone('b.jpg')));
	});

	test('a library photo never collides with a standalone image', () => {
		expect(recentId(library(1))).not.toBe(recentId(standalone('1')));
	});
});

describe('list maintenance', () => {
	test('newest lands first', () => {
		const list = withRecent(withRecent([], library(1)), library(2));
		expect(list.map(recentId)).toEqual(['library:2', 'library:1']);
	});

	test('reopening moves an entry up instead of duplicating it', () => {
		const list = withRecent(withRecent(withRecent([], library(1)), library(2)), library(1, 2000));
		expect(list.map(recentId)).toEqual(['library:1', 'library:2']);
		expect(list[0].openedAt).toBe(2000);
	});

	test('a standalone entry carries the newest doc', () => {
		const older = standalone('a.jpg');
		const newer = { ...standalone('a.jpg'), doc: { ...emptyDoc(), frames: [] }, openedAt: 5 };
		newer.doc.version = 42;
		const list = withRecent(withRecent([], older), newer);
		expect(list).toHaveLength(1);
		expect((list[0] as { doc: { version: number } }).doc.version).toBe(42);
	});

	test('the list stops at the cap', () => {
		let list: Recent[] = [];
		for (let i = 0; i < RECENTS_CAP + 5; i++) list = withRecent(list, library(i, i));
		expect(list).toHaveLength(RECENTS_CAP);
		expect(recentId(list[0])).toBe(`library:${RECENTS_CAP + 4}`);
		expect(recentId(list[RECENTS_CAP - 1])).toBe(`library:${5}`);
	});
});

describe('parsing stored recents', () => {
	test('round-trips a written list', () => {
		const list = withRecent(withRecent([], library(1)), standalone('a.jpg'));
		expect(parseRecents(JSON.stringify(list))).toEqual(list);
	});

	test('nothing stored is an empty list', () => {
		expect(parseRecents(null)).toEqual([]);
		expect(parseRecents('')).toEqual([]);
	});

	test('corrupt JSON is an empty list, not a crash', () => {
		expect(parseRecents('{ not json')).toEqual([]);
		expect(parseRecents('"a string"')).toEqual([]);
		expect(parseRecents('{"kind":"library"}')).toEqual([]);
	});

	test('unusable entries are dropped and usable ones survive', () => {
		const good = library(3);
		const raw = JSON.stringify([
			null,
			42,
			{ kind: 'library', name: 'no id', width: 1, height: 1, openedAt: 1 },
			{ kind: 'standalone', name: 'no doc', width: 1, height: 1, openedAt: 1 },
			{ kind: 'other', name: 'x', width: 1, height: 1, openedAt: 1 },
			{ kind: 'library', photoId: 1, name: 'no size', openedAt: 1 },
			good
		]);
		expect(parseRecents(raw)).toEqual([good]);
	});

	test('an oversized stored list is trimmed to the cap', () => {
		const raw = JSON.stringify(Array.from({ length: RECENTS_CAP + 9 }, (_, i) => library(i)));
		expect(parseRecents(raw)).toHaveLength(RECENTS_CAP);
	});
});

describe('relative time', () => {
	const now = Date.parse('2026-08-13T12:00:00Z');

	test('reads in the largest unit that fits', () => {
		expect(relativeTime(now - 5_000, now)).toBe('just now');
		expect(relativeTime(now - 90_000, now)).toBe('1m ago');
		expect(relativeTime(now - 3 * 3_600_000, now)).toBe('3h ago');
		expect(relativeTime(now - 2 * 86_400_000, now)).toBe('2d ago');
	});

	test('anything older than a week reads as a date', () => {
		expect(relativeTime(now - 30 * 86_400_000, now)).not.toMatch(/ago|just now/);
	});

	test('a clock that jumped backwards still reads', () => {
		expect(relativeTime(now + 60_000, now)).toBe('just now');
	});
});
