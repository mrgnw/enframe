// The preview screen's calls to the M1 routes. `/api/track` takes the pad now; `/api/export` answers with
// server-sent events that are read here by hand, because EventSource cannot POST.

import { loadFolder, photoUrl, postTrack, Stale } from '#lib/marking/api.js';
import type { Marks, Sidecar } from '#lib/marking/types.js';

export { loadFolder, photoUrl, Stale };

let sequence = 0;

/** Tracks with `marks` and `pad` (the CLI's `--pad`, none when absent); `Stale` when a newer call was made. */
export function trackPad(folder: string, marks: Marks, pad?: string): Promise<Sidecar> {
	const mine = ++sequence;
	return postTrack({ folder, marks, ...(pad && { pad }) }, () => mine === sequence);
}

export interface ExportRequest {
	folder: string;
	marks: Marks;
	out: string;
	pad?: string;
	fps: number;
	/** A directory for the aligned PNG frames. */
	frames?: string;
}

export interface Exported {
	out: string;
	sidecar: Sidecar;
}

/** Renders the clip. `onLine` sees each line pinframe prints; rejects with the server's message on failure. */
export async function exportClip(req: ExportRequest, onLine: (line: string) => void): Promise<Exported> {
	const resp = await fetch('/api/export', {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(req)
	});
	if (!resp.ok || !resp.body) {
		const body = await resp.json().catch(() => ({}));
		throw new Error(body.error ?? resp.statusText);
	}
	const reader = resp.body.pipeThrough(new TextDecoderStream()).getReader();
	let pending = '';
	for (;;) {
		const { value, done } = await reader.read();
		if (done) break;
		pending += value;
		const events = pending.split('\n\n');
		pending = events.pop() ?? '';
		for (const event of events) {
			const type = /^event: (.*)$/m.exec(event)?.[1];
			const data = JSON.parse(/^data: (.*)$/m.exec(event)?.[1] ?? 'null');
			if (type === 'progress') onLine(data.line);
			else if (type === 'done') return data;
			else if (type === 'error') throw new Error(data.error);
		}
	}
	throw new Error('the export ended without a result');
}
