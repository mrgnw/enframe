import type { AvifReply } from './avif-worker';

export const AVIF_UNAVAILABLE = 'This browser could not start the AVIF encoder — nothing was exported';

let worker: Worker | null = null;
let nextId = 1;

function startWorker(): Worker {
	if (worker) return worker;
	try {
		worker = new Worker(new URL('./avif-worker.ts', import.meta.url), { type: 'module' });
	} catch {
		throw new Error(AVIF_UNAVAILABLE);
	}
	return worker;
}

function dropWorker() {
	worker?.terminate();
	worker = null;
}

// ponytail: the pixels are transferred, so the caller's ImageData is detached once the
// encode starts. Every caller builds a throwaway crop, so nobody reads it twice.
export async function encodeAvif(pixels: ImageData, quality: number): Promise<ArrayBuffer> {
	const active = startWorker();
	const id = nextId++;

	return new Promise<ArrayBuffer>((resolve, reject) => {
		const onMessage = (event: MessageEvent<AvifReply>) => {
			if (event.data.id !== id) return;
			stop();
			const { buffer, error } = event.data;
			if (buffer) resolve(buffer);
			else reject(new Error(error ?? 'AVIF encoding failed'));
		};

		const onError = () => {
			stop();
			dropWorker();
			reject(new Error(AVIF_UNAVAILABLE));
		};

		function stop() {
			active.removeEventListener('message', onMessage);
			active.removeEventListener('error', onError);
		}

		active.addEventListener('message', onMessage);
		active.addEventListener('error', onError);
		active.postMessage({ id, data: pixels.data, width: pixels.width, height: pixels.height, quality }, [
			pixels.data.buffer
		]);
	});
}
