// Filmstrip thumbnails, made in the browser: the server has no image decoder, and the browser must decode
// each photo anyway to show it on the stage. Each photo is fetched once (the HTTP cache serves the stage
// too), scaled to the filmstrip's height by drawing it on a canvas, and kept as an object URL for the session.

const HEIGHT = 112; // 56 px at 2x
/** Photos decoding at once: all of a folder's at the same moment (57 full-size decodes) would exhaust memory. */
const DECODES = 4;

const urls = new Map<string, Promise<string>>();
const waiting: (() => void)[] = [];
let decoding = 0;

export function thumbnail(photoUrl: string): Promise<string> {
	let pending = urls.get(photoUrl);
	if (!pending) {
		pending = queued(photoUrl);
		urls.set(photoUrl, pending);
		// A failed thumbnail is made again the next time it is asked for.
		pending.catch(() => {
			if (urls.get(photoUrl) === pending) urls.delete(photoUrl);
		});
	}
	return pending;
}

/** `make`, at most `DECODES` at a time; the rest wait their turn in order. */
async function queued(photoUrl: string): Promise<string> {
	// A finished decode hands its place straight to the next in line, so the count never overshoots.
	if (decoding >= DECODES) {
		await new Promise<void>((go) => waiting.push(go));
	} else {
		decoding++;
	}
	try {
		return await make(photoUrl);
	} finally {
		const next = waiting.shift();
		if (next) next();
		else decoding--;
	}
}

async function make(photoUrl: string): Promise<string> {
	const resp = await fetch(photoUrl);
	if (!resp.ok) throw new Error(`${photoUrl}: ${resp.status}`);
	const blob = await resp.blob();
	// Scaled by drawing, not by createImageBitmap's resize options, which Safari may ignore.
	const bitmap = await createImageBitmap(blob);
	const width = Math.max(1, Math.round((bitmap.width * HEIGHT) / bitmap.height));
	const canvas = new OffscreenCanvas(width, HEIGHT);
	const context = canvas.getContext('2d');
	if (!context) throw new Error(`${photoUrl}: no 2d canvas`);
	context.imageSmoothingQuality = 'medium';
	context.drawImage(bitmap, 0, 0, width, HEIGHT);
	bitmap.close();
	const thumb = await canvas.convertToBlob({ type: 'image/jpeg', quality: 0.85 });
	return URL.createObjectURL(thumb);
}
