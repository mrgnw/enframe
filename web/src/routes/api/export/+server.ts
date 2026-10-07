import type { RequestHandler } from '@sveltejs/kit';
import {
	exportClip,
	parseMarks,
	PinframeError,
	resolveFolder,
	Superseded,
	type ExportOptions
} from '#lib/server/pinframe.js';

/**
 * POST {folder, marks, out?, pad?, fps?, frames?, vf?}: renders the clip and streams server-sent events:
 * `progress` with each line pinframe prints, then `done` with {out, sidecar}, or `error` with {error}.
 * A newer run for the same folder kills this one, which ends with `error` {superseded: true}.
 */
export const POST: RequestHandler = async ({ request }) => {
	let body: Record<string, unknown>;
	try {
		body = await request.json();
	} catch {
		return new Response(JSON.stringify({ error: 'body must be JSON: {folder, marks, out?}' }), {
			status: 400,
			headers: { 'content-type': 'application/json' }
		});
	}
	let folder: string;
	let marks;
	try {
		folder = resolveFolder(body.folder);
		marks = parseMarks(body.marks);
	} catch (e) {
		if (e instanceof PinframeError) {
			return new Response(JSON.stringify({ error: e.message }), {
				status: e.status,
				headers: { 'content-type': 'application/json' }
			});
		}
		throw e;
	}
	const options: ExportOptions = {};
	for (const key of ['out', 'pad', 'frames', 'vf'] as const) {
		if (body[key] !== undefined) {
			if (typeof body[key] !== 'string') {
				return new Response(JSON.stringify({ error: `${key} must be a string` }), {
					status: 400,
					headers: { 'content-type': 'application/json' }
				});
			}
			options[key] = body[key] as string;
		}
	}
	if (body.fps !== undefined) {
		if (typeof body.fps !== 'number' || !(body.fps > 0)) {
			return new Response(JSON.stringify({ error: 'fps must be a positive number' }), {
				status: 400,
				headers: { 'content-type': 'application/json' }
			});
		}
		options.fps = body.fps;
	}

	const encoder = new TextEncoder();
	const stream = new ReadableStream({
		async start(controller) {
			const send = (event: string, data: unknown) => {
				controller.enqueue(encoder.encode(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`));
			};
			try {
				const result = await exportClip(folder, marks, options, (stream, line) =>
					send('progress', { stream, line })
				);
				send('done', result);
			} catch (e) {
				if (e instanceof Superseded) {
					send('error', { error: e.message, superseded: true });
				} else if (e instanceof PinframeError) {
					send('error', { error: e.message });
				} else {
					send('error', { error: (e as Error).message });
				}
			}
			controller.close();
		}
	});
	return new Response(stream, {
		headers: {
			'content-type': 'text/event-stream',
			'cache-control': 'no-cache',
			connection: 'keep-alive'
		}
	});
};
