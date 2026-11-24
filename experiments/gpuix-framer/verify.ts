import assert from 'node:assert/strict';
import { mkdirSync } from 'node:fs';
import { launch } from 'gpuix-svelte/automation';
import { readPng, region } from './png.ts';

const HERE = new URL('.', import.meta.url).pathname;
const OUT = `${HERE}evidence/`;
mkdirSync(OUT, { recursive: true });

const app = await launch({ entry: 'app.ts', cwd: HERE });
const text = (id: string) => app.getByTestId(id).textContent();
const focal = async () => (await text('readout')).replace('focal ', '').split(', ').map(Number);
const cropAt = async () => (await text('crop')).replace('crop ', '').split(', ').map(Number);
const stageSize = async () => (await text('size')).replace('stage ', '').split('×').map(Number);
const near = (actual: number[], expected: number[], label: string) =>
	assert.ok(
		actual.every((v, i) => Math.abs(v - expected[i]) <= 0.005),
		`${label}: ${actual}`
	);
const within = (actual: number[], expected: number[], tolerance: number, label: string) =>
	assert.ok(
		actual.every((v, i) => Math.abs(v - expected[i]) <= tolerance),
		`${label}: ${actual} vs ${expected}`
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

	const SIDE_PANEL = 220;
	const [stageWidth] = await stageSize();
	const first = readPng(`${OUT}01-initial.png`);
	const scale = first.width / (stageWidth + SIDE_PANEL);
	assert.ok(scale >= 1, `screenshot scale ${scale}`);
	let previewBox = await app.getByTestId('preview').bounds();
	const previewPixels = async (name: string) => {
		await app.screenshot({ path: `${OUT}${name}.png` });
		return region(readPng(`${OUT}${name}.png`), previewBox, scale);
	};
	const initialPreview = region(first, previewBox, scale);

	const at = (fx: number, fy: number) => ({ x: photo.x + fx * photo.width, y: photo.y + fy * photo.height });
	const clamp01 = (v: number) => Math.min(1, Math.max(0, v));
	const expectAt = async (pointer: { x: number; y: number }, label: string, bounds = photo) => {
		const fx = clamp01((pointer.x - bounds.x) / bounds.width);
		const fy = clamp01((pointer.y - bounds.y) / bounds.height);
		near(await focal(), [fx, fy], `${label} focal`);
		const cropX = Math.min(800, Math.max(0, fx * 1200 - 200));
		const cropY = Math.min(400, Math.max(0, fy * 800 - 200));
		within(await cropAt(), [cropX, cropY], 1, `${label} crop`);
	};
	const until = async (condition: () => Promise<boolean>, label: string) => {
		for (const deadline = Date.now() + 5000; Date.now() < deadline; await settle()) {
			if (await condition()) return;
		}
		assert.fail(`timed out waiting for ${label}`);
	};
	await app.mouse.down(at(0.5, 0.5));
	const timings: number[] = [];
	const crops = new Set<string>();
	let previous = initialPreview;
	for (let i = 1; i <= 40; i++) {
		const started = performance.now();
		const pointer = at(0.5 + i * 0.01, 0.5 - i * 0.01);
		await app.mouse.move(pointer, { pressedButton: 0 });
		timings.push(performance.now() - started);
		crops.add(await text('crop'));
		if (i % 10 === 0) {
			await expectAt(pointer, `step ${i}`);
			const pixels = await previewPixels(`02-dragging-step-${i}`);
			assert.ok(!pixels.equals(previous), `preview pixels change at step ${i}`);
			previous = pixels;
		}
		await pace();
	}
	assert.match(await text('state'), /dragging/);

	const rightEdge = photo.x + photo.width;
	const rightPointer = { x: rightEdge + 300, y: photo.y + photo.height / 2 };
	await app.mouse.move(rightPointer, { pressedButton: 0 });
	await expectAt(rightPointer, 'beyond right edge');
	const pastRight = await previewPixels('03-clamped-right');
	assert.ok(!pastRight.equals(previous), 'preview changes when pointer returns from top-right to right edge');
	const furtherPointer = { x: rightEdge + 600, y: photo.y + photo.height / 2 };
	await app.mouse.move(furtherPointer, { pressedButton: 0 });
	await expectAt(furtherPointer, 'further beyond right edge');
	assert.ok((await previewPixels('03-clamped-right-further')).equals(pastRight), 'clamped preview is stable');

	const topRight = { x: rightEdge + 300, y: photo.y - 300 };
	await app.mouse.move(topRight, { pressedButton: 0 });
	await expectAt(topRight, 'beyond top-right');
	const bottomLeft = { x: photo.x - 300, y: photo.y + photo.height + 300 };
	await app.mouse.move(bottomLeft, { pressedButton: 0 });
	await expectAt(bottomLeft, 'beyond bottom-left');
	await app.screenshot({ path: `${OUT}03-clamped-bottom-left.png` });

	await app.mouse.move(at(0.25, 0.75), { pressedButton: 0 });
	await expectAt(at(0.25, 0.75), 'returns inside the photo');
	await app.mouse.up({ x: photo.x + photo.width / 2, y: photo.y - 20 });
	const released = await text('state');
	assert.match(released, /idle/);
	const releasedPreview = await previewPixels('04-released');
	const releasedMarker = await app.getByTestId('marker').bounds();
	await app.mouse.move(at(0.8, 0.2));
	near(await focal(), [0.25, 0.75], 'release outside the photo stops tracking');
	assert.equal(await text('state'), released, 'no moves counted after release');
	assert.deepEqual(await app.getByTestId('marker').bounds(), releasedMarker, 'marker stays put after release');
	assert.ok((await previewPixels('04-after-move-no-button')).equals(releasedPreview), 'preview unchanged after release');

	const sizeBefore = await stageSize();
	await app.call('keystrokes', { keys: 'z' });
	await until(async () => {
		const [w, h] = await stageSize();
		return w !== sizeBefore[0] || h !== sizeBefore[1];
	}, 'stage size change');
	await until(async () => {
		const b = await app.getByTestId('photo').bounds();
		return b.width !== photo.width || b.height !== photo.height;
	}, 'photo size change');
	const sizeAfter = await stageSize();
	assert.ok(
		sizeAfter[0] !== sizeBefore[0] || sizeAfter[1] !== sizeBefore[1],
		`stage size changed: ${sizeBefore} -> ${sizeAfter}`
	);
	const resized = await app.getByTestId('photo').bounds();
	assert.ok(
		resized.width !== photo.width || resized.height !== photo.height,
		`photo size changed: ${photo.width}×${photo.height} -> ${resized.width}×${resized.height}`
	);
	const marker = await app.getByTestId('marker').bounds();
	const centre = [
		(marker.x + marker.width / 2 - resized.x) / resized.width,
		(marker.y + marker.height / 2 - resized.y) / resized.height
	];
	console.log('after zoom', resized, await text('size'), 'marker at', centre);
	near(centre, [0.25, 0.75], 'marker anchored after resize');
	previewBox = await app.getByTestId('preview').bounds();
	await app.screenshot({ path: `${OUT}05-after-resize.png` });
	const afterResize = readPng(`${OUT}05-after-resize.png`);
	assert.ok(
		afterResize.width !== first.width || afterResize.height !== first.height,
		'screenshot size follows the window'
	);
	assert.ok(
		region(afterResize, previewBox, afterResize.width / (sizeAfter[0] + SIDE_PANEL)).equals(releasedPreview),
		'preview pixels survive resize'
	);

	const resizedCentre = { x: resized.x + resized.width / 2, y: resized.y + resized.height / 2 };
	await app.mouse.down(resizedCentre);
	await expectAt(resizedCentre, 'grab after resize', resized);
	const resizedDrag = { x: resized.x + resized.width * 0.6, y: resized.y + resized.height * 0.4 };
	await app.mouse.move(resizedDrag, { pressedButton: 0 });
	await expectAt(resizedDrag, 'drag after resize', resized);
	await app.mouse.up(resizedDrag);

	timings.sort((a, b) => a - b);
	console.log(
		`move→laid-out ms: median ${timings[20].toFixed(1)}, p95 ${timings[38].toFixed(1)}, max ${timings[39].toFixed(1)}`
	);
	console.log(`distinct crops during 40-step drag: ${crops.size}`);
	console.log('all checks passed');
} finally {
	await app.close();
}
