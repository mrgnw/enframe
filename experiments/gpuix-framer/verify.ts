import assert from 'node:assert/strict';
import { mkdirSync } from 'node:fs';
import { launch } from 'gpuix-svelte/automation';

const HERE = new URL('.', import.meta.url).pathname;
const OUT = `${HERE}evidence/`;
mkdirSync(OUT, { recursive: true });

const app = await launch({ entry: 'app.ts', cwd: HERE });
const text = (id: string) => app.getByTestId(id).textContent();
const focal = async () => (await text('readout')).replace('focal ', '').split(', ').map(Number);
const near = (actual: number[], expected: number[], label: string) =>
	assert.ok(
		actual.every((v, i) => Math.abs(v - expected[i]) <= 0.005),
		`${label}: ${actual}`
	);
const settle = () => new Promise((resolve) => setTimeout(resolve, 300));
const PACE_MS = Number(process.env.PACE_MS ?? 0);
const pace = () => new Promise((resolve) => setTimeout(resolve, PACE_MS));

try {
	await app.getByTestId('marker').waitFor({ timeoutMs: 10000 });
	await settle();
	const photo = await app.getByTestId('photo').bounds();
	console.log('photo bounds', photo, await text('size'));
	await app.screenshot({ path: `${OUT}01-initial.png` });

	const at = (fx: number, fy: number) => ({ x: photo.x + fx * photo.width, y: photo.y + fy * photo.height });
	await app.mouse.down(at(0.5, 0.5));
	const timings: number[] = [];
	const crops = new Set<string>();
	for (let i = 1; i <= 40; i++) {
		const started = performance.now();
		await app.mouse.move(at(0.5 + i * 0.01, 0.5 - i * 0.01), { pressedButton: 0 });
		timings.push(performance.now() - started);
		crops.add(await text('crop'));
		await pace();
	}
	near(await focal(), [0.9, 0.1], 'tracks while dragging');
	assert.match(await text('state'), /dragging/);
	await app.screenshot({ path: `${OUT}02-dragging-top-right.png` });

	await app.mouse.move({ x: photo.x + photo.width + 300, y: photo.y - 300 }, { pressedButton: 0 });
	near(await focal(), [1, 0], 'clamps beyond top-right');
	await app.mouse.move({ x: photo.x - 300, y: photo.y + photo.height + 300 }, { pressedButton: 0 });
	near(await focal(), [0, 1], 'clamps beyond bottom-left');
	await app.screenshot({ path: `${OUT}03-clamped-bottom-left.png` });

	await app.mouse.move(at(0.25, 0.75), { pressedButton: 0 });
	await app.mouse.up(at(0.25, 0.75));
	assert.match(await text('state'), /idle/);
	await app.mouse.move(at(0.8, 0.2));
	near(await focal(), [0.25, 0.75], 'release stops tracking');

	await app.call('keystrokes', { keys: 'z' });
	await settle();
	await settle();
	const resized = await app.getByTestId('photo').bounds();
	const marker = await app.getByTestId('marker').bounds();
	const centre = [
		(marker.x + marker.width / 2 - resized.x) / resized.width,
		(marker.y + marker.height / 2 - resized.y) / resized.height
	];
	console.log('after zoom', resized, await text('size'), 'marker at', centre);
	assert.notDeepEqual(resized, photo, 'window resized');
	near(centre, [0.25, 0.75], 'marker anchored after resize');
	await app.screenshot({ path: `${OUT}04-after-resize.png` });

	timings.sort((a, b) => a - b);
	console.log(
		`move→laid-out ms: median ${timings[20].toFixed(1)}, p95 ${timings[38].toFixed(1)}, max ${timings[39].toFixed(1)}`
	);
	console.log(`distinct crops during 40-step drag: ${crops.size}`);
	console.log('all checks passed');
} finally {
	await app.close();
}
