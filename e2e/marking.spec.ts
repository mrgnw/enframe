// pinframe's MVP walkthrough on enframe's /pin, against serve and the real pinframe binary: place the four
// reference points, Track, fix the lost photos (click the ones with a visible cloud, skip the ones where it
// is out of frame), reach "All 57 linked", then clear the fix list. The folder is the config's throwaway
// copy of the sample. Screenshots are artifacts in captures/e3/, not compared with anything.
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { expect, type Page, test } from '@playwright/test';
import {
	capture,
	clickPhoto,
	count,
	current,
	FOLDER,
	fixLost,
	type Marks,
	PINFRAME_REPO,
	photoReady,
	photoSize,
	plantDocument,
	readFeatures,
	scriptMarks,
	settled,
	WORK
} from './pin';

const SAMPLE_DOC = join(PINFRAME_REPO, 'samples', 'train clouds.pin.json');
const SHOTS = 'captures/e3';

/** The photos a person would skip: the script's mark lies within this many px of the photo's edge. */
const EDGE = 250;
const LOST_AT_FIRST = 53;
const CLICKED = [
	'IMG_4680',
	'IMG_4684',
	'IMG_4695',
	'IMG_4697',
	'IMG_4698',
	'IMG_4703',
	'IMG_4706',
	'IMG_4708',
	'IMG_4713',
	'IMG_4769',
	'IMG_4773',
	'IMG_4774',
	'IMG_4780',
	'IMG_4791',
	'IMG_4798'
].map((f) => `${f}.avif`);
const SKIPPED = [
	'IMG_4675',
	'IMG_4681',
	'IMG_4704',
	'IMG_4705',
	'IMG_4721',
	'IMG_4725',
	'IMG_4766',
	'IMG_4767',
	'IMG_4768',
	'IMG_4796'
].map((f) => `${f}.avif`);

function fingerprint(path: string): string {
	try {
		return createHash('sha1').update(readFileSync(path)).digest('hex');
	} catch {
		return 'absent';
	}
}

const shot = (page: Page, name: string) => page.screenshot({ path: `${SHOTS}/${name}.png` });
const readDoc = (): { marks: Marks } => readFeatures()[0];

const sampleBefore = fingerprint(SAMPLE_DOC);

test.afterAll(() => {
	expect(fingerprint(SAMPLE_DOC)).toBe(sampleBefore);
	if (WORK && !process.env.ENFRAME_E2E_KEEP) rmSync(WORK, { recursive: true, force: true });
});

test('marking: points, track, fix the lost photos, clear the fix list', async ({ page }) => {
	expect(FOLDER).not.toBe('');
	mkdirSync(SHOTS, { recursive: true });
	const known = scriptMarks();
	const errors: string[] = [];
	page.on('pageerror', (e) => errors.push(e.message));
	page.on('console', (m) => {
		if (m.type() === 'error') errors.push(m.text());
	});
	let sidecar: { run: string; photos: { file: string; source: string }[] } | null = null;
	page.on('response', async (r) => {
		if (r.url().endsWith('/api/stabilize/track') && r.ok()) sidecar = await r.json();
	});

	await page.goto(`/pin?folder=${encodeURIComponent(FOLDER)}`);
	const img = page.locator('.stage.marking .photo img');
	await expect(img).toBeVisible();
	await page.waitForFunction(() => (document.querySelector('.stage.marking .photo img') as HTMLImageElement)?.complete);
	await shot(page, '1-fresh');
	for (const [x, y] of known.points) await clickPhoto(page, x, y);
	await page.keyboard.press('Enter');
	await expect(page.locator('.where')).toBeVisible({ timeout: 120_000 });
	await settled(page);
	expect(await count(page)).toBe(`${LOST_AT_FIRST} to fix`);
	const first = await current(page);
	await photoReady(page, first);
	await shot(page, '2-lost-selected');

	// At zoom 3 a click in the corner would put the ghost's points outside the photo: refused.
	await page.keyboard.press('1');
	for (let i = 0; i < 4; i++) await page.keyboard.press(']');
	await expect(page.locator('.tools .val').last()).toHaveText('3×');
	await clickPhoto(page, 2, 2);
	await expect(page.locator('.photo')).toHaveClass(/blocked/);
	await expect(page.locator('.toast')).toContainText('outside');
	await expect(page.locator('.trail.ghost .pt.blocked')).not.toHaveCount(0);
	await expect(page.locator('.looking .label', { hasText: 'Looking for' })).toBeVisible();
	await capture(page, '1-edge-refused');
	expect(first in (readDoc().marks.at ?? {})).toBe(false);
	await page.keyboard.press('ArrowRight');
	await page.keyboard.press('ArrowLeft');

	const { clicked, skipped } = await fixLost(
		page,
		(file, [w, h]) => {
			const mark = known.at[file];
			if (!mark || mark.points.some(([x, y]) => x < EDGE || y < EDGE || x > w - EDGE || y > h - EDGE)) return null;
			const cx = mark.points.reduce((t, p) => t + p[0], 0) / mark.points.length;
			const cy = mark.points.reduce((t, p) => t + p[1], 0) / mark.points.length;
			return [cx, cy];
		},
		() => readDoc().marks
	);
	console.log(`clicked ${clicked.length}: ${clicked.join(' ')}\nskipped ${skipped.length}: ${skipped.join(' ')}`);
	await expect(page.locator('.count')).toHaveText('All 57 linked');
	expect(clicked).toEqual(CLICKED);
	expect(skipped).toEqual(SKIPPED);

	// Back on a marked photo: it stays in the fix list until "Clear fixed".
	const label = page.locator('.looking .label');
	for (let i = 0; i < 30 && !((await label.textContent()) ?? '').includes('Your mark'); i++) {
		await page.keyboard.press('Shift+Tab');
	}
	await expect(label).toContainText('Your mark');
	await expect(page.locator('.toast')).toHaveCount(0, { timeout: 10_000 });
	await shot(page, '3-marked');
	await expect(page.locator('.thumb.listed')).toHaveCount(clicked.length + skipped.length);
	await page.click('.btn.clear-fixed');
	await expect(page.locator('.thumb.listed')).toHaveCount(0);
	await expect(page.locator('.btn.clear-fixed')).toHaveCount(0);
	await shot(page, '4-fix-list-cleared');

	const marks = readDoc().marks;
	expect(Object.keys(marks.at ?? {}).sort()).toEqual([...CLICKED].sort());
	expect([...(marks.skip ?? [])].sort()).toEqual([...SKIPPED].sort());
	const last = sidecar as { run: string; photos: { file: string; source: string }[] } | null;
	expect(last).not.toBeNull();
	const sources: Record<string, number> = {};
	for (const p of last?.photos ?? []) sources[p.source] = (sources[p.source] ?? 0) + 1;
	console.log(`last sidecar ${last?.run}: ${JSON.stringify(sources)}`);
	expect(sources.lost ?? 0).toBe(0);
	expect(errors).toEqual([]);
});

test('a saved mark outside its photo opens to Clear, Soft mark and Skip', async ({ page }) => {
	const known = scriptMarks();
	const bad = Object.keys(known.at)[0];
	const plant = () =>
		plantDocument({ ...known, at: { ...known.at, [bad]: { points: [[-346, 200], ...known.at[bad].points] } } });
	const open = async (named: string) => {
		await page.goto(`/pin?folder=${encodeURIComponent(FOLDER)}`);
		await expect(page.locator('.drop p')).toContainText(`${named}: point -346,200 is outside`, { timeout: 120_000 });
	};
	const retracked = async () => {
		await expect(page.locator('.count')).toBeVisible({ timeout: 120_000 });
		await settled(page);
		return readDoc().marks;
	};

	plant();
	await open(bad);
	await expect(page.locator('.drop .actions button')).toHaveText(['Clear mark', 'Soft mark', 'Skip photo']);
	await capture(page, '2-bad-mark');
	await page.click('button:text("Clear mark")');
	expect(bad in ((await retracked()).at ?? {})).toBe(false);

	plant();
	await open(bad);
	await page.click('button:text("Skip photo")');
	const skipped = await retracked();
	expect(bad in (skipped.at ?? {})).toBe(false);
	expect(skipped.skip).toContain(bad);

	// Soft mark keeps the points inside the photo, moved from `at` to `near`.
	plant();
	await open(bad);
	await page.click('button:text("Soft mark")');
	const softened = await retracked();
	expect(bad in (softened.at ?? {})).toBe(false);
	expect(softened.near?.[bad]).toEqual(known.at[bad]);

	// On the first photo the points are the reference: only Clear, which goes back to the Points step.
	plantDocument({ ...known, points: [[-346, 200], ...known.points.slice(1)] });
	await open('IMG_4667.avif');
	await expect(page.locator('.drop .actions button')).toHaveText(['Clear mark']);
	await capture(page, '3-bad-first-mark');
	await page.click('button:text("Clear mark")');
	await expect(page.locator('.stage.marking')).toBeVisible();
	await photoReady(page, 'IMG_4667.avif');
	for (const [x, y] of known.points) await clickPhoto(page, x, y);
	await page.keyboard.press('Enter');
	const placed = (await retracked()).points;
	expect(placed).toHaveLength(known.points.length);
	placed.forEach(([x, y], i) => {
		expect(Math.abs(x - known.points[i][0])).toBeLessThan(2);
		expect(Math.abs(y - known.points[i][1])).toBeLessThan(2);
	});
});

test('a soft mark on a lost photo is placed, cleared and kept across a reload', async ({ page }) => {
	plantDocument(scriptMarks());
	type Sidecar = { photos: { file: string; source: string }[] };
	/** The source the next sidecar gives `file`, after `act` posts the document. */
	const sourceAfter = async (file: string, act: () => Promise<unknown>) => {
		const response = page.waitForResponse((r) => r.url().endsWith('/api/stabilize/track') && r.ok());
		await act();
		const sidecar = (await (await response).json()) as Sidecar;
		return sidecar.photos.find((r) => r.file === file)?.source;
	};
	const back = async (file: string) => {
		for (let i = 0; i < 10 && (await current(page)) !== file; i++) await page.keyboard.press('Shift+Tab');
		expect(await current(page)).toBe(file);
	};

	// A document with the script's marks opens already tracked, on the first photo it leaves lost.
	const opened = page.waitForResponse((r) => r.url().endsWith('/api/stabilize/track') && r.ok());
	await page.goto(`/pin?folder=${encodeURIComponent(FOLDER)}`);
	const lost = ((await (await opened).json()) as Sidecar).photos.filter((r) => r.source === 'lost').map((r) => r.file);
	expect(lost.length).toBeGreaterThan(0);
	await expect(page.locator('.count')).toHaveText(`${lost.length} to fix`);
	await settled(page);
	const file = await current(page);
	expect(file).toBe(lost[0]);
	await photoReady(page, file);

	// Shift-click places a soft mark where a click would place a mark.
	const [w, h] = await photoSize(page);
	await page.keyboard.down('Shift');
	const placed = await sourceAfter(file, () => clickPhoto(page, w / 2, h / 2));
	await page.keyboard.up('Shift');
	expect(placed).toBe('soft');
	expect(readDoc().marks.near?.[file]).toBeDefined();
	expect(file in (readDoc().marks.at ?? {})).toBe(false);
	await settled(page);
	await back(file);
	await expect(page.locator('.ptag.soft')).toContainText('soft mark');
	await capture(page, '4-soft-mark');

	expect(await sourceAfter(file, () => page.keyboard.press('Backspace'))).toBe('lost');
	expect(readDoc().marks.near?.[file]).toBeUndefined();
	await settled(page);

	// N accepts the estimate as a soft mark; a reload keeps it.
	expect(await sourceAfter(file, () => page.keyboard.press('n'))).toBe('soft');
	await settled(page);
	expect(await sourceAfter(file, () => page.reload())).toBe('soft');
	await settled(page);
	expect(readDoc().marks.near?.[file]).toBeDefined();
	await expect(page.locator('.thumb.soft')).toHaveCount(1);
	await capture(page, '5-soft-mark-reloaded');
});
