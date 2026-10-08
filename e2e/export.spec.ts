// pinframe's preview and export on enframe's /pin/export, against serve and the real pinframe binary: open the
// preview of a folder marked with the train clouds job, change the padding, export the clip into the config's
// throwaway work folder, and check its sidecar against the CLI's own run of the same document. Screenshots are
// artifacts in captures/e4/. The document is removed afterwards, so the marking walkthrough starts fresh.
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { chromium, expect, type Page, test } from '@playwright/test';
import { capture, count, settled } from './pin';

type Point = [number, number];
interface Marks {
	points: Point[];
	at: Record<string, { points: Point[]; zoom?: number }>;
}
interface Row {
	file: string;
	x: number;
	y: number;
	w: number;
	h: number;
	source: string;
}

const FOLDER = process.env.ENFRAME_E2E_FOLDER ?? '';
const WORK = process.env.ENFRAME_E2E_WORK ?? '';
const PINFRAME_REPO = process.env.ENFRAME_E2E_PINFRAME_REPO ?? '';
const PINFRAME_BIN = process.env.PINFRAME_BIN ?? join(PINFRAME_REPO, 'rust/target/release/pinframe');
const DOCUMENT = `${FOLDER}.pin.json`;
const OUT = join(WORK, 'e4 clip');
const REMOTE_OUT = join(WORK, 'r1 remote clip');
const CLI_OUT = join(WORK, 'e4 cli.json');
const SHOTS = 'captures/e4';

const parsePoints = (text: string): Point[] => text.split('/').map((p) => p.split(',').map(Number) as Point);
const formatPoints = (points: Point[]) => points.map(([x, y]) => `${x},${y}`).join('/');

/** The marks of the train clouds job, from pinframe's analysis/train_clouds_all.sh. */
function scriptMarks(): Marks {
	const script = readFileSync(join(PINFRAME_REPO, 'analysis', 'train_clouds_all.sh'), 'utf8');
	const points = /--points (\S+)/.exec(script)?.[1];
	if (!points) throw new Error('no --points in analysis/train_clouds_all.sh');
	const at: Marks['at'] = {};
	for (const m of script.matchAll(/^\s*--at (\S+\.avif)=([^@\s]+)(?:@([\d.]+))?$/gm)) {
		at[m[1]] = { points: parsePoints(m[2]), ...(m[3] && { zoom: Number(m[3]) }) };
	}
	if (Object.keys(at).length < 20) throw new Error('could not read the --at marks of train_clouds_all.sh');
	return { points: parsePoints(points), at };
}

/** The CLI's flags for `marks`, as serve builds them. */
function cliArgs(marks: Marks): string[] {
	const args = ['--points', formatPoints(marks.points)];
	for (const [file, mark] of Object.entries(marks.at).sort(([a], [b]) => (a < b ? -1 : 1))) {
		args.push('--at', `${file}=${formatPoints(mark.points)}${mark.zoom === undefined ? '' : `@${mark.zoom}`}`);
	}
	return args;
}

const rects = (rows: Row[]) => rows.map(({ file, x, y, w, h, source }) => ({ file, x, y, w, h, source }));
const shot = (page: Page, name: string) => page.screenshot({ path: `${SHOTS}/${name}.png` });

async function drawn(page: Page) {
	await expect(page.locator('#size')).toHaveText(/^\d+×\d+$/);
	await expect(page.locator('#transport.busy')).toHaveCount(0);
}

test.afterAll(() => {
	rmSync(DOCUMENT, { force: true });
});

test('export: preview a marked folder, change the padding, export, match the CLI', async ({ page }) => {
	test.setTimeout(600_000);
	expect(FOLDER).not.toBe('');
	mkdirSync(SHOTS, { recursive: true });
	const marks = scriptMarks();
	writeFileSync(DOCUMENT, JSON.stringify({ version: 1, folder: FOLDER, marks }, null, '\t'));
	const errors: string[] = [];
	page.on('pageerror', (e) => errors.push(e.message));
	page.on('console', (m) => {
		if (m.type() === 'error') errors.push(m.text());
	});
	const tracks: string[] = [];
	page.on('request', (r) => {
		if (r.url().endsWith('/api/pinframe/track')) tracks.push(JSON.parse(r.postData() ?? '{}').pad ?? 'none');
	});

	await page.goto(`/pin/export?folder=${encodeURIComponent(FOLDER)}`);
	await expect(page.locator('.clip canvas')).toBeVisible({ timeout: 120_000 });
	await drawn(page);
	await expect(page.locator('#play')).toContainText('❚');
	await expect(page.locator('.filmstrip .thumb')).toHaveCount(57);
	expect(tracks).toEqual(['none']);
	await shot(page, '1-preview');

	await page.click('.swatch.black');
	await expect.poll(() => tracks).toEqual(['none', 'black']);
	await drawn(page);
	await page.click('.swatch.transparent');
	await expect(page.locator('.clip')).toHaveClass(/checker/);
	await page.click('.swatch.white');
	await expect(page.locator('.clip')).not.toHaveClass(/checker/);
	await page.click('.swatch.transparent');
	expect(tracks).toEqual(['none', 'black']);
	await expect(page.locator('#format button', { hasText: 'MP4' })).toBeDisabled();
	await shot(page, '2-padded');

	await page.fill('.export .field .path', OUT);
	await page.click('#export-btn');
	await expect(page.locator('#progress')).toBeVisible();
	await shot(page, '3-exporting');
	await expect(page.locator('#done')).toBeVisible({ timeout: 480_000 });
	await expect(page.locator('#done b')).toHaveText('Wrote e4 clip.avif');
	await page.waitForFunction(() => {
		const img = document.querySelector('#clip-shown') as HTMLImageElement | null;
		return !!img && img.complete && img.naturalWidth > 0;
	});
	await expect(page.locator('#download')).toBeVisible();
	await expect(page.locator('#sidecar')).toBeVisible();
	await expect(page.locator('#reveal')).toBeVisible();
	await shot(page, '4-done');
	expect(existsSync(`${OUT}.avif`) && existsSync(`${OUT}.json`)).toBe(true);

	const sidecarResponse = await page.request.get((await page.locator('#sidecar').getAttribute('href')) ?? 'missing');
	expect(sidecarResponse.ok()).toBe(true);
	const exported: { photos: Row[] } = await sidecarResponse.json();

	const photos = readdirSync(FOLDER)
		.filter((f) => f.endsWith('.avif') && !f.startsWith('.'))
		.sort()
		.map((f) => join(FOLDER, f));
	execFileSync(PINFRAME_BIN, [...photos, ...cliArgs(marks), '--pad', 'transparent', '--out', CLI_OUT], {
		stdio: 'ignore'
	});
	const cli: { photos: Row[] } = JSON.parse(readFileSync(CLI_OUT, 'utf8'));
	expect(exported.photos).toHaveLength(57);
	expect(rects(exported.photos)).toEqual(rects(cli.photos));

	// Back to Fix on the same folder, then the phone layout of both screens.
	const lost = exported.photos.filter((r) => r.source === 'lost').length;
	await page.click('.step:has-text("Fix")');
	await expect(page.locator('.where')).toBeVisible({ timeout: 120_000 });
	await settled(page);
	expect(await count(page)).toBe(`${lost} to fix`);
	await page.setViewportSize({ width: 390, height: 844 });
	await capture(page, '6-fix-phone');
	await page.goto(`/pin/export?folder=${encodeURIComponent(FOLDER)}`);
	await expect(page.locator('#play')).toContainText('❚', { timeout: 120_000 });
	await capture(page, '7-preview-phone');
	expect(errors).toEqual([]);
});

test('export: Reveal in Finder is offered only on a local instance', async ({ baseURL }) => {
	const remote = new URL(baseURL ?? '');
	remote.hostname = 'enframe.test';
	const browser = await chromium.launch({ args: [`--host-resolver-rules=MAP ${remote.hostname} 127.0.0.1`] });
	try {
		const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
		await page.goto(new URL(`/pin/export?folder=${encodeURIComponent(FOLDER)}`, remote).href);
		await expect(page.locator('.clip canvas')).toBeVisible({ timeout: 120_000 });
		await drawn(page);
		await page.fill('.export .field .path', REMOTE_OUT);
		await page.click('#export-btn');
		await expect(page.locator('#done')).toBeVisible({ timeout: 480_000 });
		await page.waitForFunction(() => {
			const img = document.querySelector('#clip-shown') as HTMLImageElement | null;
			return !!img && img.complete && img.naturalWidth > 0;
		});
		await expect(page.locator('#download')).toBeVisible();
		await expect(page.locator('#sidecar')).toBeVisible();
		await expect(page.locator('#reveal')).toHaveCount(0);
		await capture(page, '8-done-remote');
	} finally {
		await browser.close();
	}
});
