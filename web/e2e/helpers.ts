import { readFileSync } from 'node:fs';
import { expect, type Page } from '@playwright/test';
import { SHOTS } from './sandbox.ts';
import type { Marks } from './marks.ts';

export const shot = (page: Page, name: string) => page.screenshot({ path: `${SHOTS}/${name}.png` });

export const readDoc = (path: string): { marks: Marks; photos: string[] } => JSON.parse(readFileSync(path, 'utf8'));

/** Waits until the marking screen has no track in flight. */
export async function settled(page: Page) {
	await expect(page.locator('.island.busy')).toHaveCount(0);
}

export const count = async (page: Page) => ((await page.locator('.count').textContent()) ?? '').trim();

/** The photo the marking screen is on, as the document names it: `IMG_4675.avif` (the screen shows the stem). */
export async function current(page: Page): Promise<string> {
	const where = (await page.locator('.where').textContent()) ?? '';
	const name = /(\S+)\s*·/.exec(where.replace(/\s+/g, ' '));
	if (!name) throw new Error(`cannot read the photo name from "${where}"`);
	return `${name[1]}.avif`;
}

/**
 * Waits until the stage shows `file` fully decoded and its box has stopped moving. A click before that lands
 * on the previous photo's box (a point outside the photo, which the server then refuses to track).
 */
export async function photoReady(page: Page, file: string) {
	await page.waitForFunction(
		(f) => {
			const img = document.querySelector('.stage .photo img') as HTMLImageElement | null;
			return !!img && img.complete && img.naturalWidth > 0 && decodeURIComponent(img.src).includes(`file=${f}`);
		},
		file
	);
	let last = '';
	await expect
		.poll(async () => {
			const b = await page.locator('.photo').boundingBox();
			const now = JSON.stringify(b);
			const same = now === last;
			last = now;
			return same;
		}, { intervals: [100] })
		.toBe(true);
}

/** Clicks a point given in photo pixels (the oriented size the CLI uses) on the stage's photo. */
export async function clickPhoto(page: Page, x: number, y: number) {
	const img = page.locator('.stage .photo img');
	const [w, h] = await img.evaluate((i: HTMLImageElement) => [i.naturalWidth, i.naturalHeight]);
	const box = await page.locator('.photo').boundingBox();
	if (!box) throw new Error('no photo on the stage');
	await page.mouse.click(box.x + (box.width * x) / w, box.y + (box.height * y) / h);
	return [w, h];
}
