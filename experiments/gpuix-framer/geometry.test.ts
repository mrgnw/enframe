import assert from 'node:assert/strict';
import { crop, toFocal } from './geometry.ts';

const photo = { x: 0, y: 100, width: 600, height: 400 };
assert.deepEqual(toFocal(photo, 300, 300), { x: 0.5, y: 0.5 });
assert.deepEqual(toFocal(photo, -50, 9999), { x: 0, y: 1 });
assert.deepEqual(toFocal({ ...photo, width: 0 }, 0, 200), { x: 0.5, y: 0.25 });
assert.deepEqual(toFocal({ ...photo, height: 0 }, 150, 100), { x: 0.25, y: 0.5 });
assert.deepEqual(crop({ x: 0.5, y: 0.5 }, 1200, 800, 400), { x: 400, y: 200, width: 400, height: 400 });
assert.deepEqual(crop({ x: 1, y: 0 }, 1200, 800, 400), { x: 800, y: 0, width: 400, height: 400 });
console.log('geometry ok');
