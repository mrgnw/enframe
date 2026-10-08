// The browser tests: `bun run test:e2e` (builds the web app first). Playwright starts serve itself, on a
// port and database of its own, rooted on a throwaway copy of pinframe's train clouds sample, so the real
// samples are never written. Screenshots go to captures/e3/ (gitignored).
import { cpSync, existsSync, mkdirSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { defineConfig } from '@playwright/test';

const PINFRAME_REPO = process.env.PINFRAME_REPO ?? '/Users/user/dev/pinframe';
const SAMPLES = join(PINFRAME_REPO, 'samples', 'train clouds');
const port = Number(process.env.ENFRAME_E2E_PORT ?? 17591);
const baseURL = `http://127.0.0.1:${port}`;

// Workers load this file again: the runner's work folder is passed down through the environment.
process.env.ENFRAME_E2E_WORK ??= mkdtempSync(join(tmpdir(), 'enframe-e2e-'));
const work = process.env.ENFRAME_E2E_WORK;
const folder = join(work, 'train clouds');
if (!existsSync(folder)) cpSync(SAMPLES, folder, { recursive: true });
mkdirSync(join(work, 'inbox'), { recursive: true });
process.env.ENFRAME_E2E_FOLDER = folder;
process.env.ENFRAME_E2E_PINFRAME_REPO = PINFRAME_REPO;

export default defineConfig({
	testDir: 'e2e',
	workers: 1,
	fullyParallel: false,
	retries: 0,
	timeout: 300_000,
	expect: { timeout: 60_000 },
	reporter: [['list']],
	outputDir: 'captures/e3/results',
	use: {
		baseURL,
		actionTimeout: 20_000,
		viewport: { width: 1280, height: 800 }
	},
	projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
	webServer: {
		command: [
			'bun run serve',
			`--port ${port}`,
			'--web build',
			`--db "${join(work, 'enframe.db')}"`,
			`--inbox "${join(work, 'inbox')}"`,
			`--read-root "${work}"`,
			`--write-root "${work}"`
		].join(' '),
		url: baseURL,
		reuseExistingServer: false,
		timeout: 600_000
	}
});
