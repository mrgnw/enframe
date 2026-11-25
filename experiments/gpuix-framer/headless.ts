import assert from 'node:assert/strict';
import { mkdirSync } from 'node:fs';
import {
	bounds,
	mount_headless,
	mouse_down,
	mouse_move,
	mouse_up,
	screenshot,
	text_content,
	find,
	wait_for
} from 'gpuix-svelte/test';
import App from './App.svelte';

const HERE = new URL('.', import.meta.url).pathname;
mkdirSync(`${HERE}evidence`, { recursive: true });

const by = (id: string) => find((node) => node.testId === id)!;
const text = (id: string) => text_content(by(id));
const near = (actual: number[], expected: number[], label: string) =>
	assert.ok(actual.every((v, i) => Math.abs(v - expected[i]) <= 0.005), `${label}: ${actual}`);
const focal = () => text('readout').replace('focal ', '').split(', ').map(Number);

mount_headless(App, {
	props: { photo: `${HERE}photo.jpg`, photoWidth: 1200, photoHeight: 800 },
	width: 900,
	height: 520
});
await wait_for(() => !text('size').startsWith('stage 0'), { label: 'stage laid out' });
const photo = bounds(by('photo'));
const at = (fx: number, fy: number) => ({ x: photo.x + fx * photo.width, y: photo.y + fy * photo.height });

mouse_down(at(0.5, 0.5));
const crops = new Set<string>();
for (let i = 1; i <= 40; i++) {
	mouse_move(at(0.5 + i * 0.01, 0.5 - i * 0.01), { pressed_button: 0 });
	crops.add(text('crop'));
}
near(focal(), [0.9, 0.1], 'tracks while dragging');
assert.match(text('state'), /dragging · moves 40/);
assert.ok(crops.size > 20, `preview changes during drag: ${crops.size}`);
screenshot(`${HERE}evidence/headless-dragging.png`);

mouse_move({ x: photo.x + photo.width + 300, y: -300 }, { pressed_button: 0 });
near(focal(), [1, 0], 'clamps beyond top-right');
mouse_move({ x: -300, y: photo.y + photo.height + 300 }, { pressed_button: 0 });
near(focal(), [0, 1], 'clamps beyond bottom-left');

mouse_move(at(0.25, 0.75), { pressed_button: 0 });
mouse_up(at(0.25, 0.75));
mouse_move(at(0.8, 0.2));
assert.match(text('state'), /idle/);
near(focal(), [0.25, 0.75], 'release stops tracking');
screenshot(`${HERE}evidence/headless-released.png`);

const marker = bounds(by('marker'));
near(
	[(marker.x + marker.width / 2 - photo.x) / photo.width, (marker.y + marker.height / 2 - photo.y) / photo.height],
	[0.25, 0.75],
	'marker painted at focal point'
);
console.log('headless ok', { photo, crops: crops.size });
process.exit(0);
