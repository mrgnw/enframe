// The app switch: the wordmark each screen shows top-left doubles as the way across to the other app.
// Stabilize -> enframe by the menu, enframe -> Stabilize by the key, with the menu open screenshotted in
// both shells. Screenshots are artifacts in captures/nav/, not compared with anything.
import { mkdirSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import { FOLDER } from './stabilize';

const SHOTS = 'captures/nav';

test('the wordmark switches between enframe and Stabilize', async ({ page }) => {
	mkdirSync(SHOTS, { recursive: true });

	await page.goto(`/stabilize?folder=${encodeURIComponent(FOLDER)}`);
	const darkTrigger = page.locator('.app-switch .trigger.full');
	await expect(darkTrigger).toHaveText('Stabilize');

	await darkTrigger.click();
	const darkMenu = page.locator('.app-switch .menu');
	await expect(darkMenu).toBeVisible();
	await expect(darkMenu.locator('.item.current')).toHaveText('Stabilize');
	const toEnframe = darkMenu.locator('.item', { hasText: 'enframe' });
	await expect(toEnframe.locator('kbd')).toHaveText('⌘1');
	await page.screenshot({ path: `${SHOTS}/dark-menu.png` });

	await toEnframe.click();
	await expect(page).toHaveURL(/\/$/);
	const lightTrigger = page.locator('.app-switch .trigger.full');
	await expect(lightTrigger).toHaveText('enframe');

	await lightTrigger.click();
	const lightMenu = page.locator('.app-switch .menu');
	await expect(lightMenu).toBeVisible();
	await expect(lightMenu.locator('.item.current')).toHaveText('enframe');
	await page.screenshot({ path: `${SHOTS}/light-menu.png` });
	await page.keyboard.press('Escape');
	await expect(lightMenu).toBeHidden();

	await page.keyboard.press('Meta+2');
	await expect(page).toHaveURL(/\/stabilize$/);
});
