import { execFile } from 'node:child_process';
import { stat } from 'node:fs/promises';
import { json, type RequestHandler } from '@sveltejs/kit';
import { CAN_REVEAL, PinframeError, resolveClip } from '#lib/server/pinframe.js';

/**
 * POST {path}: shows a clip in Finder (`open -R`), on the machine the server runs on. Only the Mac has
 * Finder, so elsewhere this answers 501 and the done state does not offer it.
 */
export const POST: RequestHandler = async ({ request }) => {
	if (!CAN_REVEAL) return json({ error: 'reveal needs the server to run on macOS' }, { status: 501 });
	let path: string;
	try {
		const body = await request.json().catch(() => ({}));
		path = await resolveClip(body.path);
	} catch (e) {
		if (e instanceof PinframeError) return json({ error: e.message }, { status: e.status });
		throw e;
	}
	try {
		await stat(path);
	} catch {
		return json({ error: `${path}: not found` }, { status: 404 });
	}
	return new Promise<Response>((done) => {
		execFile('open', ['-R', path], (error) =>
			done(error ? json({ error: `open -R: ${error.message}` }, { status: 500 }) : json({ ok: true }))
		);
	});
};
