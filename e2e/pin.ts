// What the pinframe walkthroughs share: the config's throwaway folder, the train clouds job's known marks, and
// the moves on the marking screen (read the photo on screen, click a point given in photo pixels, fix the lost
// photos one by one).
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, type Page } from '@playwright/test';

export type Point = [number, number];
export interface Marks {
	points: Point[];
	at?: Record<string, { points: Point[]; zoom?: number }>;
	near?: Record<string, { points: Point[]; zoom?: number }>;
	skip?: string[];
}
export interface Feature {
	name: string;
	marks: Marks;
}

export const FOLDER = process.env.ENFRAME_E2E_FOLDER ?? '';
export const WORK = process.env.ENFRAME_E2E_WORK ?? '';
export const PINFRAME_REPO = process.env.ENFRAME_E2E_PINFRAME_REPO ?? '';
export const DOCUMENT = `${FOLDER}.pin.json`;

const parsePoints = (text: string): Point[] => text.split('/').map((p) => p.split(',').map(Number) as Point);

/** The known marks of the train clouds job, from pinframe's analysis/train_clouds_all.sh. */
export function scriptMarks(): Required<Pick<Marks, 'points' | 'at'>> {
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

/** The saved document's features; a read that catches the file mid-rename is retried by the caller's poll. */
export const readFeatures = (): Feature[] => JSON.parse(readFileSync(DOCUMENT, 'utf8')).features;

/** Writes the folder's document by hand, as one feature: what a person or another tool could leave there. */
export const plantDocument = (marks: Marks) =>
	writeFileSync(DOCUMENT, `${JSON.stringify({ version: 2, features: [{ name: 'Feature 1', marks }] }, null, '\t')}\n`);

/** A screenshot of a state the R1 parity tests added, in captures/r1/. */
export async function capture(page: Page, name: string) {
	mkdirSync('captures/r1', { recursive: true });
	await page.screenshot({ path: `captures/r1/${name}.png` });
}

export const count = async (page: Page) => ((await page.locator('.count').textContent()) ?? '').trim();

export async function settled(page: Page) {
	await expect(page.locator('.island.busy')).toHaveCount(0);
}

/** The photo the screen is on, as the document names it: the screen shows the stem. */
export async function current(page: Page): Promise<string> {
	const where = (await page.locator('.where').textContent()) ?? '';
	const name = /(\S+)\s*·/.exec(where.replace(/\s+/g, ' '));
	if (!name) throw new Error(`cannot read the photo name from "${where}"`);
	return `${name[1]}.avif`;
}

/** Waits until the stage shows `file` decoded and its box has stopped moving. */
export async function photoReady(page: Page, file: string) {
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

/** The stage photo's natural size. */
export const photoSize = (page: Page) =>
	page.locator('.stage .photo img').evaluate((i: HTMLImageElement) => [i.naturalWidth, i.naturalHeight]);

/** Clicks a point given in photo pixels on the stage's photo. */
export async function clickPhoto(page: Page, x: number, y: number) {
	const [w, h] = await photoSize(page);
	const box = await page.locator('.photo').boundingBox();
	if (!box) throw new Error('no photo on the stage');
	await page.mouse.click(box.x + (box.width * x) / w, box.y + (box.height * y) / h);
}

/**
 * Fixes the lost photos as they come: `decide` names where to click on a photo (photo pixels), or null to
 * skip it. Each answer is saved before the next photo; `marks` reads the feature's saved marks.
 */
export async function fixLost(
	page: Page,
	decide: (file: string, size: number[]) => Point | null,
	marks: () => Marks,
	rounds = 40
): Promise<{ clicked: string[]; skipped: string[] }> {
	const clicked: string[] = [];
	const skipped: string[] = [];
	for (let round = 0; round < rounds; round++) {
		await settled(page);
		if (!(await count(page)).includes('to fix')) break;
		const file = await current(page);
		await photoReady(page, file);
		await expect(page.locator('.looking .label', { hasText: 'Looking for' })).toBeVisible();
		const at = decide(file, await photoSize(page));
		if (at) {
			await clickPhoto(page, ...at);
			clicked.push(file);
		} else {
			await page.keyboard.press('s');
			skipped.push(file);
		}
		// Serve saves the document before it tracks.
		await expect
			.poll(() => {
				try {
					const m = marks();
					return file in (m.at ?? {}) || (m.skip ?? []).includes(file);
				} catch {
					return false;
				}
			})
			.toBe(true);
		await expect.poll(async () => !(await count(page)).includes('to fix') || (await current(page)) !== file).toBe(true);
	}
	await settled(page);
	return { clicked, skipped };
}
