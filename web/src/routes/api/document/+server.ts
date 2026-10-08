import { basename } from 'node:path';
import { json, type RequestHandler } from '@sveltejs/kit';
import { listPhotos, loadDocument, PinframeError, resolveFolder } from '#lib/server/pinframe.js';

/**
 * GET ?folder=: the folder's photos by name and its saved document (`<folder>.pin.json`), or null when
 * it has none yet, so the marking screen knows whether to ask for the first photo's points or to track.
 */
export const GET: RequestHandler = async ({ url }) => {
	try {
		const folder = resolveFolder(url.searchParams.get('folder'));
		const photos = (await listPhotos(folder)).map((p) => basename(p));
		return json({ folder, photos, marks: await loadDocument(folder) });
	} catch (e) {
		if (e instanceof PinframeError) return json({ error: e.message }, { status: e.status });
		throw e;
	}
};
