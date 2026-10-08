// pinframe's MVP walkthrough on enframe's /pin, against serve and the real pinframe binary: place the four
// reference points, Track, fix the lost photos (click the ones with a visible cloud, skip the ones where it
// is out of frame), reach "All 57 linked", then clear the fix list. The folder is the config's throwaway
// copy of the sample. Screenshots are artifacts in captures/e3/, not compared with anything.
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { expect, type Page, test } from '@playwright/test';

type Point = [number, number];
interface Marks {
	points: Point[];
	at?: Record<string, { points: Point[]; zoom?: number }>;
	skip?: string[];
}

const FOLDER = process.env.ENFRAME_E2E_FOLDER ?? '';
const WORK = process.env.ENFRAME_E2E_WORK ?? '';
const PINFRAME_REPO = process.env.ENFRAME_E2E_PINFRAME_REPO ?? '';
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

const parsePoints = (text: string): Point[] => text.split('/').map((p) => p.split(',').map(Number) as Point);

/** The known marks of the train clouds job, from pinframe's analysis/train_clouds_all.sh. */
function scriptMarks(): Required<Pick<Marks, 'points' | 'at'>> {
	const script = readFileSync(join(PINFRAME_REPO, 'analysis', 'train_clouds_all.sh'), 'utf8');
	const points = /--points (\S+)/.exec(script)?.[1];
	if (!points) throw new Error('no --points in analysis/train_clouds_all.sh');
	const at: NonNullable<Marks['at']> = {};
	for (const m of script.matchAll(/^\s*--at (\S+\.avif)=([^@\s]+)(?:@([\d.]+))?$/gm)) {
		at[m[1]] = { points: parsePoints(m[2]), ...(m[3] && { zoom: Number(m[3]) }) };
	}
	if (Object.keys(at).length < 20) throw new Error('could not read the --at marks of train_clouds_all.sh');
	return { points: parsePoints(points), at };
}

function fingerprint(path: string): string {
	try {
		return createHash('sha1').update(readFileSync(path)).digest('hex');
	} catch {
		return 'absent';
	}
}

const shot = (page: Page, name: string) => page.screenshot({ path: `${SHOTS}/${name}.png` });
const readDoc = (): { marks: Marks } => JSON.parse(readFileSync(`${FOLDER}.pin.json`, 'utf8'));
const count = async (page: Page) => ((await page.locator('.count').textContent()) ?? '').trim();

async function settled(page: Page) {
	await expect(page.locator('.island.busy')).toHaveCount(0);
}

/** The photo the screen is on, as the document names it: the screen shows the stem. */
async function current(page: Page): Promise<string> {
	const where = (await page.locator('.where').textContent()) ?? '';
	const name = /(\S+)\s*·/.exec(where.replace(/\s+/g, ' '));
	if (!name) throw new Error(`cannot read the photo name from "${where}"`);
	return `${name[1]}.avif`;
}

/** Waits until the stage shows `file` decoded and its box has stopped moving. */
async function photoReady(page: Page, file: string) {
	await page.waitForFunction((f) => {
		const img = document.querySelector('.stage .photo img') as HTMLImageElement | null;
		return !!img && img.complete && img.naturalWidth > 0 && decodeURIComponent(img.src).endsWith(`/${f}`);
	}, file);
	let last = '';
	await expect
		.poll(
			async () => {
				const now = JSON.stringify(await page.locator('.photo').boundingBox());
				const same = now === last;
				last = now;
				return same;
			},
			{ intervals: [100] }
		)
		.toBe(true);
}

/** Clicks a point given in photo pixels on the stage's photo. */
async function clickPhoto(page: Page, x: number, y: number) {
	const [w, h] = await page
		.locator('.stage .photo img')
		.evaluate((i: HTMLImageElement) => [i.naturalWidth, i.naturalHeight]);
	const box = await page.locator('.photo').boundingBox();
	if (!box) throw new Error('no photo on the stage');
	await page.mouse.click(box.x + (box.width * x) / w, box.y + (box.height * y) / h);
}

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

	const clicked: string[] = [];
	const skipped: string[] = [];
	for (let round = 0; round < 40; round++) {
		await settled(page);
		if (!(await count(page)).includes('to fix')) break;
		const file = await current(page);
		await photoReady(page, file);
		await expect(page.locator('.looking .label', { hasText: 'Looking for' })).toBeVisible();
		const mark = known.at[file];
		const [w, h] = await page
			.locator('.stage .photo img')
			.evaluate((i: HTMLImageElement) => [i.naturalWidth, i.naturalHeight]);
		const near = !mark || mark.points.some(([x, y]) => x < EDGE || y < EDGE || x > w - EDGE || y > h - EDGE);
		if (near) {
			await page.keyboard.press('s');
			skipped.push(file);
		} else {
			const cx = mark.points.reduce((t, p) => t + p[0], 0) / mark.points.length;
			const cy = mark.points.reduce((t, p) => t + p[1], 0) / mark.points.length;
			await clickPhoto(page, cx, cy);
			clicked.push(file);
		}
		// Serve saves the document before it tracks.
		await expect
			.poll(() => {
				try {
					const m = readDoc().marks;
					return file in (m.at ?? {}) || (m.skip ?? []).includes(file);
				} catch {
					return false;
				}
			})
			.toBe(true);
		await expect.poll(async () => !(await count(page)).includes('to fix') || (await current(page)) !== file).toBe(true);
	}
	await settled(page);
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
