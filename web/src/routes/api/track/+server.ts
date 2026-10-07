import { json, type RequestHandler } from '@sveltejs/kit';
import { parseMarks, PinframeError, resolveFolder, Superseded, track } from '#lib/server/pinframe.js';

/**
 * POST {folder, marks}: writes the marks to `<folder>.pin.json`, runs pinframe JSON-only with the
 * feature cache, and returns the sidecar. A newer request for the same folder kills this one, which then
 * answers 409; the sidecar's `run` tells a client which answer is the newest.
 */
export const POST: RequestHandler = async ({ request }) => {
	let body: { folder?: unknown; marks?: unknown };
	try {
		body = await request.json();
	} catch {
		return json({ error: 'body must be JSON: {folder, marks}' }, { status: 400 });
	}
	try {
		const folder = resolveFolder(body.folder);
		const marks = parseMarks(body.marks);
		return json(await track(folder, marks));
	} catch (e) {
		if (e instanceof Superseded) {
			return json({ error: e.message, superseded: true }, { status: 409 });
		}
		if (e instanceof PinframeError) {
			return json({ error: e.message }, { status: e.status });
		}
		throw e;
	}
};
