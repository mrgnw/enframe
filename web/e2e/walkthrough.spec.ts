// The MVP walkthrough on a throwaway copy of the sample folder: open it, place the four reference points,
// Track, fix the lost photos (click the ones with a visible cloud, skip the ones where it is out of frame),
// reach "All 57 linked", Preview, Export, and compare the exported sidecar to the CLI's.
// Screenshots are artifacts in out/e2e/shots/, not compared with the mockups: the mockup is a scenario,
// not the real tracker's output.
import { existsSync, mkdirSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { cliSidecar, readSidecar, scriptSidecar, worstDifference } from './compare.ts';
import { clickPhoto, count, current, photoReady, readDoc, settled, shot } from './helpers.ts';
import { scriptMarks, type Marks } from './marks.ts';
import { sandbox, SHOTS, type Sandbox } from './sandbox.ts';

test.describe.configure({ mode: 'serial' });
mkdirSync(SHOTS, { recursive: true });

/** The photos a person would skip: the script's mark lies within this many px of the photo's edge (cloud out of frame). */
const EDGE = 250;
/** Fixed by the tracker, which check.sh holds to the committed baseline: 53 photos lost after the first Track. */
const LOST_AT_FIRST = 53;
/** What the loop below does with the 53 lost photos; the tracker is deterministic, so these are fixed. */
const CLICKED = [
	'IMG_4680', 'IMG_4684', 'IMG_4695', 'IMG_4697', 'IMG_4698', 'IMG_4703', 'IMG_4706', 'IMG_4708',
	'IMG_4713', 'IMG_4769', 'IMG_4773', 'IMG_4774', 'IMG_4780', 'IMG_4791', 'IMG_4798'
].map((f) => `${f}.avif`);
const SKIPPED = [
	'IMG_4675', 'IMG_4681', 'IMG_4704', 'IMG_4705', 'IMG_4721', 'IMG_4725', 'IMG_4766', 'IMG_4767',
	'IMG_4768', 'IMG_4796'
].map((f) => `${f}.avif`);

let box: Sandbox;
const started = Date.now();
const lap = (what: string) => console.log(`  ${((Date.now() - started) / 1000).toFixed(1).padStart(6)} s  ${what}`);
test.afterAll(async () => box?.dispose());

async function openAndTrack(page: import('@playwright/test').Page, folder: string, ref: Marks) {
	await page.goto('/');
	await page.fill('input[name=folder]', folder);
	await page.click('button[type=submit]');
	const img = page.locator('.stage.marking .photo img');
	await expect(img).toBeVisible();
	await page.waitForFunction(() => (document.querySelector('.stage.marking .photo img') as HTMLImageElement)?.complete);
	await shot(page, '1-points-empty');
	for (const [x, y] of ref.points) await clickPhoto(page, x, y);
	await shot(page, '2-points-placed');
	await page.keyboard.press('Enter');
	await expect(page.locator('.where')).toBeVisible();
	await settled(page);
}

test('walkthrough: mark, track, fix 25 lost photos, preview, export', async ({ page }) => {
	box = await sandbox();
	console.log(`sandbox: ${box.linked}, ${box.ms.toFixed(0)} ms`);
	const known = scriptMarks();
	const errors: string[] = [];
	page.on('pageerror', (e) => errors.push(e.message));
	page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));

	await openAndTrack(page, box.folder, known);
	lap('first Track done');
	expect(await count(page)).toBe(`${LOST_AT_FIRST} to fix`);
	await shot(page, '3-tracked-first-lost');

	// At zoom 3 a click in the corner would put the ghost's points outside the photo: refused, cursor shows it.
	const edge = await current(page);
	await photoReady(page, edge);
	await page.keyboard.press('1');
	for (let i = 0; i < 4; i++) await page.keyboard.press(']');
	await expect(page.locator('.tools .val').last()).toHaveText('3×');
	await clickPhoto(page, 2, 2);
	await expect(page.locator('.photo')).toHaveClass(/blocked/);
	await expect(page.locator('.toast')).toContainText('outside');
	expect(edge in (readDoc(box.doc).marks.at ?? {})).toBe(false);
	await expect(page.locator('.looking .label', { hasText: 'Looking for' })).toBeVisible();
	// Leaving and coming back resets the zoom to the reference's, untouched, for the loop below.
	await page.keyboard.press('ArrowRight');
	await page.keyboard.press('ArrowLeft');

	// Fix loop: wait for the last re-track to land, then act on whichever photo auto-advance put on screen,
	// the way a person would. Its patch and ghost size from the sidecar's confirmed scale, not a locally
	// set one, so this is safe even right after advancing. Click the script's mark centre, or skip when it
	// is near the photo's edge.
	const clicked: string[] = [];
	const skipped: string[] = [];
	for (let round = 0; round < 40; round++) {
		await settled(page);
		if (!(await count(page)).includes('to fix')) break;
		const file = await current(page);
		await photoReady(page, file);
		await expect(page.locator('.looking .label', { hasText: 'Looking for' })).toBeVisible();
		const mark = known.at[file];
		const dims = await page.locator('.stage .photo img').evaluate((i: HTMLImageElement) => [i.naturalWidth, i.naturalHeight]);
		const near = !mark || mark.points.some(([x, y]) => x < EDGE || y < EDGE || x > dims[0] - EDGE || y > dims[1] - EDGE);
		if (near) {
			await page.keyboard.press('s');
			skipped.push(file);
		} else {
			const cx = mark.points.reduce((t, p) => t + p[0], 0) / mark.points.length;
			const cy = mark.points.reduce((t, p) => t + p[1], 0) / mark.points.length;
			await clickPhoto(page, cx, cy);
			clicked.push(file);
		}
		// The document on disk has the change (the server saves it before it tracks).
		await expect
			.poll(() => {
				try {
					const m = readDoc(box.doc).marks;
					return file in (m.at ?? {}) || (m.skip ?? []).includes(file);
				} catch {
					return false; // caught mid-write: the server rewrites the file in place
				}
			})
			.toBe(true);
		// Auto-advance moves on 450 ms after the action, possibly after the re-track is already in.
		await expect
			.poll(async () => !(await count(page)).includes('to fix') || (await current(page)) !== file)
			.toBe(true);
	}
	await settled(page);
	console.log(`clicked ${clicked.length}: ${clicked.join(' ')}\nskipped ${skipped.length}: ${skipped.join(' ')}`);
	await expect(page.locator('.count')).toHaveText('All 57 linked');
	await shot(page, '4-all-linked');
	lap('all linked');
	expect(clicked).toEqual(CLICKED);
	expect(skipped).toEqual(SKIPPED);

	// Preview: the clip plays from the sidecar.
	await page.click('a.btn.primary:text("Preview")');
	await page.waitForURL(/\/preview\?folder=/);
	await expect(page.locator('#play')).toContainText('❚', { timeout: 120_000 });
	await expect(page.locator('#size')).toContainText('×');
	await shot(page, '5-preview');
	lap('preview playing');

	// Export to a temp path beside the folder, never inside it.
	const base = join(box.out, 'walk');
	await page.fill('.export .path', base);
	await page.click('#export-btn');
	await expect(page.locator('#done')).toBeVisible({ timeout: 240_000 });
	await shot(page, '6-exported');
	lap('exported');
	expect(existsSync(`${base}.avif`) && existsSync(`${base}.json`)).toBe(true);

	// Back to Fix on the same folder, then the phone layout of both screens.
	await page.click('.step:text("Fix")');
	await expect(page.locator('.where')).toBeVisible();
	await settled(page);
	await expect(page.locator('.count')).toHaveText('All 57 linked');
	await page.setViewportSize({ width: 390, height: 844 });
	await shot(page, '7-fix-phone');
	await page.goto(`/preview?folder=${encodeURIComponent(box.folder)}`);
	await expect(page.locator('#play')).toContainText('❚', { timeout: 120_000 });
	await shot(page, '8-preview-phone');

	// The exported sidecar against a CLI run with the same marks: the document the app saved.
	const marks = readDoc(box.doc).marks;
	expect(Object.keys(marks.at ?? {}).length).toBe(clicked.length);
	expect((marks.skip ?? []).length).toBe(skipped.length);
	const worst = worstDifference(readSidecar(`${base}.json`), cliSidecar(box.folder, marks, join(box.out, 'cli.json')));
	console.log(`exported sidecar vs CLI: max difference ${worst}`);
	lap('compared');
	expect(errors).toEqual([]);
});

test('a saved mark outside its photo opens to Clear and Skip', async ({ page }) => {
	const other = await sandbox();
	try {
		const known = scriptMarks();
		const photos = readdirSync(other.folder).filter((f) => f.endsWith('.avif')).sort();
		const bad = Object.keys(known.at)[0];
		const plant = () => {
			const at = { ...known.at, [bad]: { points: [[-346, 200], ...known.at[bad].points] as [number, number][] } };
			writeFileSync(other.doc, JSON.stringify({ version: 1, folder: other.folder, marks: { ...known, at }, photos }, null, '\t') + '\n');
		};
		const open = async () => {
			await page.goto(`/?folder=${encodeURIComponent(other.folder)}`);
			await expect(page.locator('.drop p')).toContainText(`${bad}: point -346,200 is outside`, { timeout: 120_000 });
		};
		plant();
		await open();
		await expect(page.locator('.drop .actions button')).toHaveText(['Clear mark', 'Skip photo']);
		await shot(page, '10-bad-mark');
		await page.click('button:text("Clear mark")');
		await expect(page.locator('.count')).toBeVisible({ timeout: 120_000 });
		await settled(page);
		expect(bad in (readDoc(other.doc).marks.at ?? {})).toBe(false);

		plant();
		await open();
		await page.click('button:text("Skip photo")');
		await expect(page.locator('.count')).toBeVisible({ timeout: 120_000 });
		await settled(page);
		const marks = readDoc(other.doc).marks;
		expect(bad in (marks.at ?? {})).toBe(false);
		expect(marks.skip).toContain(bad);
	} finally {
		await other.dispose();
	}
});

test('baseline marks: the app agrees with analysis/train_clouds_all.sh', async ({ page }) => {
	// A saved document holding the script's own marks opens already tracked. Its export must match the
	// script's run (the one check.sh holds to the committed baseline) exactly, zoom marks included.
	const other = await sandbox();
	try {
		const known = scriptMarks();
		const photos = readdirSync(other.folder).filter((f) => f.endsWith('.avif')).sort();
		writeFileSync(other.doc, JSON.stringify({ version: 1, folder: other.folder, marks: known, photos }, null, '\t') + '\n');
		const script = scriptSidecar(join(other.out, 'script.json'));
		const lost = script.filter((r) => r.source === 'lost').map((r) => r.file);
		expect(lost.length).toBeGreaterThan(0); // the script leaves a few photos to the estimate; check.sh prints them
		await page.goto(`/?folder=${encodeURIComponent(other.folder)}`);
		await expect(page.locator('.count')).toHaveText(`${lost.length} to fix`, { timeout: 120_000 });
		await settled(page);
		expect(await current(page)).toBe(lost[0]);
		await shot(page, '9-baseline-marks-open');
		await page.click('a.btn.primary:text("Preview")');
		await expect(page.locator('#play')).toContainText('❚', { timeout: 120_000 });
		const base = join(other.out, 'baseline');
		await page.fill('.export .path', base);
		await page.click('#export-btn');
		await expect(page.locator('#done')).toBeVisible({ timeout: 240_000 });
		const worst = worstDifference(readSidecar(`${base}.json`), script);
		console.log(`exported sidecar vs train_clouds_all.sh: max difference ${worst}`);
		lap('baseline marks compared');
	} finally {
		await other.dispose();
	}
});
