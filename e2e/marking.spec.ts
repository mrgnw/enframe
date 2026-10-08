// pinframe's MVP walkthrough on enframe's /pin, against serve and the real pinframe binary: place the four
// reference points, Track, fix the lost photos (click the ones with a visible cloud, skip the ones where it
// is out of frame), reach "All 57 linked", then clear the fix list. The folder is the config's throwaway
// copy of the sample. Screenshots are artifacts in captures/e3/, not compared with anything.
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { expect, type Page, test } from '@playwright/test';
import {
	clickPhoto,
	count,
	current,
	FOLDER,
	fixLost,
	type Marks,
	PINFRAME_REPO,
	photoReady,
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
		if (r.url().endsWith('/api/pinframe/track') && r.ok()) sidecar = await r.json();
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
