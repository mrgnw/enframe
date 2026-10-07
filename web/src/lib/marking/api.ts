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

/** Tracks with `marks`; resolves with the sidecar, or rejects with `Stale` when a newer call was made. */
export async function track(folder: string, marks: Marks): Promise<Sidecar> {
	const mine = ++sequence;
	const resp = await fetch('/api/track', {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ folder, marks })
	});
	if (mine !== sequence || resp.status === 409) throw new Stale();
	const body = await resp.json();
	if (!resp.ok) throw new Error(body.error ?? resp.statusText);
	return body;
}
