/// <reference types="@sveltejs/kit" />
/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />

import { build, files, version } from '$service-worker';

const worker = self as unknown as ServiceWorkerGlobalScope;

// One cache per deploy; activation drops the rest.
const CACHE = `enframe-${version}`;

// The SPA fallback page answers every navigation, and build already carries the wasm
// codecs — precaching them means a RAW opened for the first time offline still develops.
const SHELL = '/';
const ASSETS = [SHELL, ...build, ...files];

// The page showing install progress is exactly the client the spec says this worker
// does not control yet, so progress rides a broadcast rather than a client message.
const channel = typeof BroadcastChannel === 'undefined' ? null : new BroadcastChannel('enframe-offline');

// The codec workers live outside the build list (see scripts/worker-manifest.mjs), and
// they are the difference between "the page reloads offline" and "a RAW still develops
// offline" — but a missing manifest must not cost the shell its cache.
async function workerAssets(): Promise<string[]> {
	try {
		const response = await fetch('/worker-assets.json');
		if (response.ok) return (await response.json()) as string[];
	} catch {
		// Codecs will be cached on first use instead.
	}
	return [];
}

// addAll would do this in one opaque step; fetching by hand is what buys a progress
// count. Hashed assets can come from the HTTP cache, but the shell and the static
// files keep their names across deploys, so they must skip it or a reinstall could
// seal in the stale copy.
// Only one AVIF encoder can ever run: jsquash takes the multithreaded build exactly
// when SharedArrayBuffer exists, which is a property of the origin's isolation headers
// and so reads the same here as on the page. The other build is 3.4 MiB that no fetch
// will ever ask for — and if this reading is ever wrong, the runtime cache picks the
// right one up on first use.
function usable(assets: string[]): string[] {
	const spare = worker.crossOriginIsolated ? 'assets/avif_enc-' : 'avif_enc_mt';
	return assets.filter((asset) => !asset.includes(spare));
}

async function precache(): Promise<void> {
	const cache = await caches.open(CACHE);
	const assets = [...ASSETS, ...usable(await workerAssets())];
	const queue = [...assets];
	let done = 0;
	async function pump(): Promise<void> {
		for (;;) {
			const asset = queue.shift();
			if (asset === undefined) return;
			const fresh = asset.startsWith('/_app/immutable/') ? undefined : ('reload' as const);
			const response = await fetch(asset, { cache: fresh });
			if (!response.ok) throw new Error(`precache ${asset}: ${response.status}`);
			await cache.put(asset, response);
			done += 1;
			channel?.postMessage({ type: 'progress', done, total: assets.length });
		}
	}
	await Promise.all(Array.from({ length: 6 }, pump));
	channel?.postMessage({ type: 'done' });
}

worker.addEventListener('install', (event) => {
	event.waitUntil(precache().then(() => worker.skipWaiting()));
});

worker.addEventListener('activate', (event) => {
	event.waitUntil(
		caches
			.keys()
			.then((keys) => Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key))))
			.then(() => worker.clients.claim())
	);
});

worker.addEventListener('fetch', (event) => {
	const { request } = event;
	if (request.method !== 'GET') return;
	const url = new URL(request.url);
	if (url.origin !== location.origin) return;

	// Every route is the same shell. The network's comes first: a cached one can belong to
	// the last deploy, and its chunks go with that deploy's cache the moment this deploy's
	// worker activates, which leaves a blank page or a screen without its styles. Whatever
	// the network answers is passed on, a redirect included (it arrives opaque, not ok);
	// only no network at all falls back to the cached shell, which is what makes an
	// offline reload work.
	if (request.mode === 'navigate') {
		event.respondWith(fetch(request).catch(async () => (await caches.match(SHELL)) ?? Response.error()));
		return;
	}

	// Hashed assets never change under their name, so cache-first costs nothing, and one
	// missed at install (a codec worker, say) is kept once fetched; anything else (the
	// /img and /api routes of a connected instance) stays a plain network call.
	const immutable = url.pathname.startsWith('/_app/immutable/');
	event.respondWith(
		caches.match(request).then(async (cached) => {
			if (cached) return cached;
			const response = await fetch(request);
			if (immutable && response.ok) {
				const copy = response.clone();
				void caches.open(CACHE).then((cache) => cache.put(request, copy));
			}
			return response;
		})
	);
});
