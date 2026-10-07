// The browser tests: `pnpm test:e2e` (builds first), or `pnpm exec playwright test` after `pnpm build`.
// Playwright starts the built server itself, on its own port so a dev server on 7341 and check.sh's
// round-trip port 17341 are left alone. Screenshots and the run's work folders go to out/e2e/ (gitignored).
import { defineConfig } from '@playwright/test';

const port = Number(process.env.PINFRAME_E2E_PORT ?? 17342);
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
	testDir: 'e2e',
	// One server, one pinframe run per folder, and the tests share the CPU: one worker, in order.
	workers: 1,
	fullyParallel: false,
	retries: 0,
	timeout: 240_000,
	expect: { timeout: 60_000 },
	reporter: [['list']],
	outputDir: '../out/e2e/results',
	use: {
		baseURL,
		actionTimeout: 20_000,
		viewport: { width: 1280, height: 800 },
		// Headless is Playwright's default on every platform; no flags.
	},
	projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
	webServer: {
		command: 'node build',
		url: baseURL,
		env: { PORT: String(port), HOST: '127.0.0.1' },
		reuseExistingServer: false,
		timeout: 30_000
	}
});
