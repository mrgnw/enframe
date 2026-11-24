import assert from 'node:assert/strict';
import { crop, fit, toFocal } from './geometry.ts';

const photo = fit({ x: 0, y: 0, width: 600, height: 600 }, 1200, 800);
assert.deepEqual(photo, { x: 0, y: 100, width: 600, height: 400 });
assert.deepEqual(toFocal(photo, 300, 300), { x: 0.5, y: 0.5 });
assert.deepEqual(toFocal(photo, -50, 9999), { x: 0, y: 1 });
assert.deepEqual(crop({ x: 0.5, y: 0.5 }, 1200, 800, 400), { x: 400, y: 200, width: 400, height: 400 });
assert.deepEqual(crop({ x: 1, y: 0 }, 1200, 800, 400), { x: 800, y: 0, width: 400, height: 400 });
console.log('geometry ok');
