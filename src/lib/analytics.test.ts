import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';
import { expect, test } from 'vitest';

const html = readFileSync(new URL('../app.html', import.meta.url), 'utf8');
const bootstrap = html.match(/<script>([\s\S]*?)<\/script>/)?.[1];

function load(hostname: string) {
	const scripts: { defer?: boolean; src?: string; dataset: Record<string, string> }[] = [];
	const window: Record<string, unknown> = {};
	expect(bootstrap).toBeDefined();
	runInNewContext(bootstrap!, {
		window,
		location: { hostname },
		document: {
			createElement: () => ({ dataset: {} }),
			head: { appendChild: (script: (typeof scripts)[number]) => scripts.push(script) }
		}
	});
	return scripts;
}

test('loads the deferred collector on the public web app', () => {
	expect(load('enframe.xces.workers.dev')).toEqual([
		{
			defer: true,
			src: 'https://traks-collect.xces.workers.dev/t.js',
			dataset: { site: 'pb_live_rcycb2i775b2jvnk2ui1vj7o' }
		}
	]);
});

test.each([
	'localhost',
	'127.0.0.1',
	'tauri.localhost',
	'',
	'preview.enframe.xces.workers.dev'
])('does not send analytics from %s', (hostname) => {
	expect(load(hostname)).toEqual([]);
});
