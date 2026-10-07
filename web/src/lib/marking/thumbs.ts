// Filmstrip thumbnails, made in the browser: the server has no image decoder, and the browser must decode
// each photo anyway to show it on the stage. Each photo is fetched once (the HTTP cache serves the stage
// too), scaled to the filmstrip's height with createImageBitmap, and kept as an object URL for the session.

const HEIGHT = 112; // 56 px at 2x

const urls = new Map<string, Promise<string>>();

export function thumbnail(photoUrl: string): Promise<string> {
	let pending = urls.get(photoUrl);
	if (!pending) {
		pending = make(photoUrl);
		urls.set(photoUrl, pending);
	}
	return pending;
}

async function make(photoUrl: string): Promise<string> {
	const resp = await fetch(photoUrl);
	if (!resp.ok) throw new Error(`${photoUrl}: ${resp.status}`);
	const blob = await resp.blob();
	const bitmap = await createImageBitmap(blob, { resizeHeight: HEIGHT, resizeQuality: 'medium' });
	const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
	canvas.getContext('2d')!.drawImage(bitmap, 0, 0);
	bitmap.close();
	const thumb = await canvas.convertToBlob({ type: 'image/jpeg', quality: 0.85 });
	return URL.createObjectURL(thumb);
}
