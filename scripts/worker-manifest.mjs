// $service-worker's `build` list only carries the main-thread bundle; the worker
// bundles (RAW + AVIF codecs and their wasm) are a separate Vite build it never sees.
// Their names are content-hashed, so the build writes them to a manifest the service
// worker precaches from at install.
import { readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const out = process.argv[2] ?? 'build-web';
const workers = join(out, '_app/immutable/workers');

function walk(dir) {
	return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
		entry.isDirectory() ? walk(join(dir, entry.name)) : [join(dir, entry.name)]
	);
}

const assets = walk(workers).map((path) => `/${path.slice(out.length + 1)}`);
writeFileSync(join(out, 'worker-assets.json'), JSON.stringify(assets));
console.log(`worker-assets.json: ${assets.length} files`);
