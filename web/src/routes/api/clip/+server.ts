import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { extname } from 'node:path';
import { Readable } from 'node:stream';
import { json, type RequestHandler } from '@sveltejs/kit';
import { CLIP_TYPES, PinframeError, resolveClip } from '#lib/server/pinframe.js';

/**
 * A `Range` header's first range as [start, end] inclusive, null when there is none to honour (absent, not
 * bytes, several ranges: the whole file is the answer), or 'unsatisfiable' when it lies past the end.
 */
function byteRange(header: string | null, size: number): [number, number] | 'unsatisfiable' | null {
	const m = /^bytes=(\d*)-(\d*)$/.exec(header?.trim() ?? '');
	if (!m || (m[1] === '' && m[2] === '')) return null;
	let start: number;
	let end: number;
	if (m[1] === '') {
		// `-500`: the last 500 bytes.
		start = Math.max(0, size - Number(m[2]));
		end = size - 1;
	} else {
		start = Number(m[1]);
		end = m[2] === '' ? size - 1 : Math.min(Number(m[2]), size - 1);
	}
	return start >= size || start > end ? 'unsatisfiable' : [start, end];
}

/**
 * GET ?path=: a clip or its sidecar, for the done state to show and download. Cached by size and mtime like
 * the photos; video answers byte ranges (206), which Safari needs before it plays anything.
 */
export const GET: RequestHandler = async ({ url, request }) => {
	let path: string;
	try {
		path = await resolveClip(url.searchParams.get('path'));
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
	if (!info.isFile()) return json({ error: `${path}: not a file` }, { status: 404 });
	const etag = `"${info.size}-${Math.floor(info.mtimeMs)}"`;
	const headers = {
		etag,
		'accept-ranges': 'bytes',
		'cache-control': 'private, max-age=3600',
		'content-type': CLIP_TYPES[extname(path).toLowerCase()] ?? 'application/octet-stream'
	};
	if (request.headers.get('if-none-match') === etag) {
		return new Response(null, { status: 304, headers });
	}
	// A range is honoured unless `If-Range` names another version of the file, which gets it whole.
	const ifRange = request.headers.get('if-range');
	const range = ifRange && ifRange !== etag ? null : byteRange(request.headers.get('range'), info.size);
	if (range === 'unsatisfiable') {
		return new Response(null, { status: 416, headers: { ...headers, 'content-range': `bytes */${info.size}` } });
	}
	if (range) {
		const [start, end] = range;
		const stream = Readable.toWeb(createReadStream(path, { start, end })) as ReadableStream;
		return new Response(stream, {
			status: 206,
			headers: { ...headers, 'content-range': `bytes ${start}-${end}/${info.size}`, 'content-length': String(end - start + 1) }
		});
	}
	const stream = Readable.toWeb(createReadStream(path)) as ReadableStream;
	return new Response(stream, { headers: { ...headers, 'content-length': String(info.size) } });
};
