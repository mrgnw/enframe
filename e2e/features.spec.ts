// Two features in one folder, on enframe's /stabilize and /stabilize/export against serve and the real tracker: the
// clouds marked as the marking walkthrough does, then a second feature on the mountain ridge, which the tracker
// cannot follow from the first photo alone, so it is marked on IMG_4698 and every photo it loses is skipped.
// Switching keeps both, a reload reads both back, and "Export all" writes two clips whose sidecars match the
// CLI's own runs of each feature. Screenshots are artifacts in captures/stabilize/e6/.
import { execFileSync } from 'node:child_process';
import { mkdirSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { expect, type Page, test } from '@playwright/test';
import {
	clickPhoto,
	current,
	DOCUMENT,
	FOLDER,
	fixLost,
	type Marks,
	photoReady,
	readFeatures,
	scriptMarks,
	STABILIZE_BIN,
	settled,
	WORK
} from './stabilize';

interface Row {
	file: string;
	x: number;
	y: number;
	w: number;
	h: number;
	source: string;
}

const SHOTS = 'captures/stabilize/e6';
const EDGE = 250;
/** Textured stretches of the ridge: on the first photo, and on IMG_4698 beside the wind turbine. */
const RIDGE_POINTS: [number, number][] = [
	[1450, 935],
	[1550, 935],
	[1650, 940]
];
const RIDGE_MARK = { file: 'IMG_4698.avif', at: [1600, 1005] as [number, number] };
const RIDGE = 'Ridge';

const formatPoints = (points: number[][]) => points.map(([x, y]) => `${x},${y}`).join('/');

/** The CLI's flags for `marks`, as serve builds them. */
function cliArgs(marks: Marks): string[] {
	const args = ['--points', formatPoints(marks.points)];
	for (const [flag, map] of [
		['--at', marks.at],
		['--near', marks.near]
	] as const) {
		for (const [file, mark] of Object.entries(map ?? {}).sort(([a], [b]) => (a < b ? -1 : 1))) {
			args.push(flag, `${file}=${formatPoints(mark.points)}${mark.zoom === undefined ? '' : `@${mark.zoom}`}`);
		}
	}
	for (const file of marks.skip ?? []) args.push('--skip', file);
	return args;
}

const rects = (rows: Row[]) => rows.map(({ file, x, y, w, h, source }) => ({ file, x, y, w, h, source }));
const shot = (page: Page, name: string) => page.screenshot({ path: `${SHOTS}/${name}.png` });
const names = (page: Page) => page.locator('#features li').allTextContents();
const marksOf = (name: string) => () => {
	const feature = readFeatures().find((f) => f.name === name);
	if (!feature) throw new Error(`${name} is not saved yet`);
	return feature.marks;
};

test.afterAll(() => {
	rmSync(DOCUMENT, { force: true });
});

test('features: mark two features, switch, reload, export all, match the CLI', async ({ page }) => {
	test.setTimeout(1_200_000);
	expect(FOLDER).not.toBe('');
	mkdirSync(SHOTS, { recursive: true });
	rmSync(DOCUMENT, { force: true });
	const known = scriptMarks();
	const errors: string[] = [];
	page.on('pageerror', (e) => errors.push(e.message));
	page.on('console', (m) => {
		if (m.type() === 'error') errors.push(m.text());
	});
	const tracks: string[] = [];
	let refused = 0;
	page.on('request', (r) => {
		if (r.url().endsWith('/api/stabilize/track')) tracks.push(JSON.parse(r.postData() ?? '{}').feature);
	});
	page.on('response', async (r) => {
		if (r.url().endsWith('/api/stabilize/track') && /fewer than two photos/.test(await r.text().catch(() => ''))) {
			refused++;
		}
	});

	// The first feature, as the marking walkthrough marks it.
	await page.goto(`/stabilize?folder=${encodeURIComponent(FOLDER)}`);
	await expect.poll(() => names(page)).toEqual(['Feature 1']);
	await photoReady(page, 'IMG_4667.avif');
	for (const [x, y] of known.points) await clickPhoto(page, x, y);
	await expect(page.locator('.island.end .muted')).toHaveText('4 points');
	await page.keyboard.press('Enter');
	await expect(page.locator('.where')).toBeVisible({ timeout: 120_000 });
	await fixLost(
		page,
		(file, [w, h]) => {
			const mark = known.at[file];
			if (!mark || mark.points.some(([x, y]) => x < EDGE || y < EDGE || x > w - EDGE || y > h - EDGE)) return null;
			const cx = mark.points.reduce((t, p) => t + p[0], 0) / mark.points.length;
			const cy = mark.points.reduce((t, p) => t + p[1], 0) / mark.points.length;
			return [cx, cy];
		},
		marksOf('Feature 1')
	);
	await expect(page.locator('.count')).toHaveText('All 57 linked');
	const clouds = marksOf('Feature 1')();
	await shot(page, '1-first-feature');

	// A second feature: the Points step on the first photo, then the ridge, which only the first photo holds.
	await page.click('#new-feature');
	await expect.poll(() => names(page)).toEqual(['Feature 1', 'Feature 2']);
	await expect(page.locator('.stage.marking')).toBeVisible();
	await photoReady(page, 'IMG_4667.avif');
	for (const [x, y] of RIDGE_POINTS) await clickPhoto(page, x, y);
	await shot(page, '2-ridge-points');
	await page.keyboard.press('Enter');
	await expect(page.locator('.count')).toHaveText('56 to fix');
	expect(refused).toBe(1);

	while ((await current(page)) !== RIDGE_MARK.file) await page.keyboard.press('ArrowRight');
	await photoReady(page, RIDGE_MARK.file);
	await clickPhoto(page, ...RIDGE_MARK.at);
	await expect.poll(() => RIDGE_MARK.file in (marksOf('Feature 2')().at ?? {})).toBe(true);
	await settled(page);
	await shot(page, '3-ridge-marked');
	const { skipped } = await fixLost(page, () => null, marksOf('Feature 2'), 70);
	console.log(`ridge: skipped ${skipped.length}`);
	await expect(page.locator('.count')).toHaveText('All 57 linked');
	await page.click('#rename-feature');
	await page.fill('#features .name', RIDGE);
	await page.keyboard.press('Enter');
	await expect.poll(() => names(page)).toEqual(['Feature 1', RIDGE]);
	await expect.poll(() => readFeatures().map((f) => f.name)).toEqual(['Feature 1', RIDGE]);
	await settled(page);
	const ridge = marksOf(RIDGE)();
	expect(Object.keys(ridge.at ?? {})).toEqual([RIDGE_MARK.file]);
	expect(marksOf('Feature 1')()).toEqual(clouds);

	// Switching shows each feature's own result, and neither re-tracks: both sidecars are held in the page.
	const before = tracks.length;
	await page.keyboard.press('{');
	await expect(page.locator('#features li.on')).toHaveText('Feature 1');
	await expect(page.locator('.count')).toHaveText('All 57 linked');
	await expect(page.locator('.stage')).not.toHaveClass(/tinted/);
	await shot(page, '4-switched-to-clouds');
	await page.keyboard.press('}');
	await expect(page.locator('#features li.on')).toHaveText(RIDGE);
	await expect(page.locator('.stage')).toHaveClass(/tinted/);
	expect(tracks.length).toBe(before);

	// A reload reads both features back; the second tracks when it is first shown.
	await page.reload();
	await expect.poll(() => names(page)).toEqual(['Feature 1', RIDGE]);
	await expect(page.locator('.count')).toHaveText('All 57 linked', { timeout: 120_000 });
	await page.keyboard.press('}');
	await expect(page.locator('#features li.on')).toHaveText(RIDGE);
	await expect(page.locator('.count')).toHaveText('All 57 linked', { timeout: 120_000 });
	await settled(page);
	await shot(page, '5-reloaded');
	expect(readFeatures()).toEqual([
		{ name: 'Feature 1', marks: clouds },
		{ name: RIDGE, marks: ridge }
	]);

	// Export all: one clip per feature, at their default destinations.
	await page.click('.island.end a.btn.primary');
	await expect(page).toHaveURL(/\/stabilize\/export\?.*feature=Ridge/);
	await expect(page.locator('.clip canvas')).toBeVisible({ timeout: 120_000 });
	await expect(page.locator('#feature button.on')).toHaveText(RIDGE);
	await page.click('#feature button:has-text("Feature 1")');
	await expect(page.locator('.export .field .path')).toHaveValue(`${FOLDER}-Feature 1`);
	await expect(page.locator('#size')).toHaveText(/^\d+×\d+$/, { timeout: 120_000 });
	await page.click('#export-all');
	await expect(page.locator('#progress')).toBeVisible();
	await shot(page, '6-exporting-all');
	await expect(page.locator('#clips li')).toHaveCount(2, { timeout: 900_000 });
	await expect(page.locator('#clips li b')).toHaveText(['Feature 1', RIDGE]);
	await shot(page, '7-exported-all');

	const photos = readdirSync(FOLDER)
		.filter((f) => f.endsWith('.avif') && !f.startsWith('.'))
		.sort()
		.map((f) => join(FOLDER, f));
	for (const [i, feature] of [
		{ name: 'Feature 1', marks: clouds },
		{ name: RIDGE, marks: ridge }
	].entries()) {
		const href = await page.locator('#clips li a.sidecar').nth(i).getAttribute('href');
		const response = await page.request.get(href ?? 'missing');
		expect(response.ok()).toBe(true);
		const exported: { photos: Row[] } = await response.json();
		const cliOut = join(WORK, `e6 cli ${feature.name}.json`);
		execFileSync(STABILIZE_BIN, [...photos, ...cliArgs(feature.marks), '--out', cliOut], { stdio: 'ignore' });
		const cli: { photos: Row[] } = JSON.parse(readFileSync(cliOut, 'utf8'));
		expect(exported.photos).toHaveLength(57);
		expect(rects(exported.photos), feature.name).toEqual(rects(cli.photos));
	}
	// The ridge's first track was refused (only the first photo held it), and the browser logs that answer.
	expect(errors.filter((e) => !/status of 500/.test(e))).toEqual([]);
	expect(errors.length).toBeLessThanOrEqual(refused);
});
