import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { extname } from 'node:path';
import { Readable } from 'node:stream';
import { json, type RequestHandler } from '@sveltejs/kit';
import { PHOTO_TYPES, PinframeError, resolveFolder, resolvePhoto } from '#lib/server/pinframe.js';

/** GET ?folder=&file=: the photo's bytes, cached by size and mtime, so the browser decodes and shows it. */
export const GET: RequestHandler = async ({ url, request }) => {
	let path: string;
	try {
		path = resolvePhoto(resolveFolder(url.searchParams.get('folder')), url.searchParams.get('file'));
	} catch (e) {
		if (e instanceof PinframeError) return json({ error: e.message }, { status: e.status });
		throw e;
	}
	let info;
	try {
		info = await stat(path);
	} catch {
		return json({ error: `${path}: not found` }, { status: 404 });
	}
	const etag = `"${info.size}-${Math.floor(info.mtimeMs)}"`;
	const headers = {
		etag,
		'cache-control': 'private, max-age=3600',
		'content-type': PHOTO_TYPES[extname(path).toLowerCase()] ?? 'application/octet-stream'
	};
	if (request.headers.get('if-none-match') === etag) {
		return new Response(null, { status: 304, headers });
	}
	const stream = Readable.toWeb(createReadStream(path)) as ReadableStream;
	return new Response(stream, { headers: { ...headers, 'content-length': String(info.size) } });
};
