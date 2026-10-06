// Node + Pyodide: decode two AVIFs with @jsquash/avif, run wasm/pair.py under WASM OpenCV, compare to CPython.
import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import decode, { init as initAvif } from '@jsquash/avif/decode.js';
import { loadPyodide } from 'pyodide';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const A = join(root, 'samples/train clouds/IMG_4791.avif');
const B = join(root, 'samples/train clouds/IMG_4792.avif');
const BOX = [520, 220, 620, 500];

const ms = {};
const timed = async (name, f) => {
	const t0 = performance.now();
	const r = await f();
	ms[name] = Math.round(performance.now() - t0);
	return r;
};

const py = await timed('pyodide_boot', () => loadPyodide());
await timed('load_numpy_opencv', () => py.loadPackage(['numpy', 'opencv-python']));
const avifWasm = join(root, 'wasm/node_modules/@jsquash/avif/codec/dec/avif_dec.wasm');
await initAvif(await WebAssembly.compile(readFileSync(avifWasm)));
const [a, b] = await timed('decode_avif_x2', async () => [
	await decode(readFileSync(A).buffer),
	await decode(readFileSync(B).buffer),
]);

py.FS.writeFile('/a.rgba', new Uint8Array(a.data.buffer));
py.FS.writeFile('/b.rgba', new Uint8Array(b.data.buffer));
py.FS.writeFile('/pair.py', readFileSync(join(root, 'wasm/pair.py'), 'utf8'));
py.globals.set('shape_a', [a.height, a.width]);
py.globals.set('shape_b', [b.height, b.width]);
py.globals.set('box', BOX);

const wasm = await timed('fit', () =>
	py.runPythonAsync(`
import json, sys
sys.path.insert(0, '/')
import numpy as np, pair
def load(path, shape):
	h, w = shape
	return np.fromfile(path, np.uint8).reshape(h, w, 4)[:, :, :3].copy()
a, b = load('/a.rgba', shape_a.to_py()), load('/b.rgba', shape_b.to_py())
cold = pair.fit(a, b, tuple(box.to_py()))
warm = pair.fit(a, b, tuple(box.to_py()))
cold['ms_warm'] = warm['ms']
json.dumps(cold)
`),
).then(JSON.parse);

const native = JSON.parse(execFileSync('uv', ['run', 'wasm/reference.py'], { cwd: root, encoding: 'utf8' }));

const row = (name, w, n) => console.log(`${name.padEnd(14)} ${String(w).padEnd(28)} ${n}`);
row('', 'pyodide (wasm)', 'cpython (native)');
row('opencv', wasm.opencv, native.opencv);
row('features_box', wasm.features_box, native.features_box);
row('features_full', wasm.features_full, native.features_full);
row('pairs', wasm.pairs, native.pairs);
row('inliers', wasm.inliers, native.inliers);
row('anchor_b', wasm.anchor_b.map((v) => v.toFixed(1)).join(','), native.anchor_b.map((v) => v.toFixed(1)).join(','));
row('scale', wasm.scale.toFixed(4), native.scale.toFixed(4));
row('turn_deg', wasm.turn_deg.toFixed(2), native.turn_deg.toFixed(2));
for (const k of Object.keys(wasm.ms)) row(`ms ${k}`, `${wasm.ms[k]} (warm ${wasm.ms_warm[k]})`, native.ms[k]);
console.log('anchor.py says', JSON.stringify(native.anchor_py));
console.log('node side ms', JSON.stringify(ms));
