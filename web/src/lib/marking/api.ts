// The screen's calls to the M1 routes. Every change posts the whole document; the newest post wins, and
// an answer to an older one is dropped (the server kills the older run and answers 409, but a slow
// network can still deliver old answers out of order, so the sequence number decides).

import type { Marks, Sidecar } from './types';

export const photoUrl = (folder: string, file: string) =>
	`/api/photo?folder=${encodeURIComponent(folder)}&file=${encodeURIComponent(file)}`;

export interface Folder {
	folder: string;
	photos: string[];
	marks: Marks | null;
}

export async function loadFolder(folder: string): Promise<Folder> {
	const resp = await fetch(`/api/document?folder=${encodeURIComponent(folder)}`);
	const body = await resp.json();
	if (!resp.ok) throw new Error(body.error ?? resp.statusText);
	return body;
}

export class Stale extends Error {
	constructor() {
		super('a newer request replaced this one');
	}
}

let sequence = 0;

/**
 * Posts a track request and reads its sidecar. `newest` says whether no newer call has been made since: a
 * 409 then means the server was beaten by something this page did not send (a request that overtook it),
 * so it asks once more; otherwise, or when the answer is not the newest any more, it is `Stale`.
 */
export async function postTrack(body: object, newest: () => boolean): Promise<Sidecar> {
	const payload = JSON.stringify(body);
	for (let attempt = 0; ; attempt++) {
		const resp = await fetch('/api/track', {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: payload
		});
		if (!newest()) throw new Stale();
		if (resp.status === 409) {
			if (attempt === 0) continue;
			throw new Stale();
		}
		const answer = await resp.json();
		if (!newest()) throw new Stale();
		if (!resp.ok) throw new Error(answer.error ?? resp.statusText);
		return answer;
	}
}

/** Tracks with `marks`; resolves with the sidecar, or rejects with `Stale` when a newer call was made. */
export function track(folder: string, marks: Marks): Promise<Sidecar> {
	const mine = ++sequence;
	return postTrack({ folder, marks }, () => mine === sequence);
}
