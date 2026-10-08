// The screen's calls to the connected instance's Stabilize routes. Every change posts the whole document; the
// newest post wins, and an answer to an older one is dropped (the server kills the older run and answers 409,
// but a slow network can still deliver old answers out of order, so the sequence number decides).

import { currentConnection, getImageSrc, isLocalInstance, listDirectory, readTextFile } from '$lib/bridge';
import { type Feature, FIRST_FEATURE, type Marks, type Sidecar } from './types';

const PHOTO = /\.(avif|heic|heif|jxl|jpe?g|png)$/i;
const MISSING = /\(os error 2\)/;

const trimmed = (folder: string) => folder.replace(/(.)\/+$/, '$1');

export const photoUrl = (folder: string, file: string) => getImageSrc(`${trimmed(folder)}/${file}`);

export interface Folder {
	folder: string;
	photos: string[];
	/** Empty when the folder has no document yet. */
	features: Feature[];
}

// Serve answers a missing file and one outside the roots alike, so absence is read from the parent's listing
// first. A document that cannot be told absent is read, and its error shown: never a fresh start, since the
// first track would overwrite it.
async function absent(document: string): Promise<boolean> {
	const slash = document.lastIndexOf('/');
	const name = document.slice(slash + 1);
	const listing = await listDirectory(document.slice(0, slash) || '/').catch(() => null);
	return listing !== null && !listing.entries.some((e) => e.name === name);
}

async function readDocument(document: string): Promise<string | null> {
	if (await absent(document)) return null;
	try {
		return await readTextFile(document);
	} catch (e) {
		if (MISSING.test((e as Error).message)) return null;
		throw e;
	}
}

// A document saved as `<folder>.pin.json`, before the feature was Stabilize, is read when `<folder>.stabilize.json`
// is absent, and one from before features, `{marks}`, reads as one feature; the next save writes the new name and
// shape.
async function loadDocument(folder: string): Promise<Feature[]> {
	const text =
		(await readDocument(`${trimmed(folder)}.stabilize.json`)) ?? (await readDocument(`${trimmed(folder)}.pin.json`));
	if (text === null) return [];
	const saved = JSON.parse(text) as { features?: Feature[]; marks?: Marks };
	return saved.features ?? (saved.marks ? [{ name: FIRST_FEATURE, marks: saved.marks }] : []);
}

export async function loadFolder(folder: string): Promise<Folder> {
	const listing = await listDirectory(folder);
	const photos = listing.entries
		.filter((e) => !e.isDirectory && PHOTO.test(e.name))
		.map((e) => e.name)
		.sort();
	return { folder, photos, features: await loadDocument(folder) };
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
		const resp = await fetch(`${currentConnection()}/api/stabilize/track`, {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: payload
		});
		if (!newest()) throw new Stale();
		if (resp.status === 409) {
			if (attempt === 0) continue;
			throw new Stale();
		}
		const answer = await resp.text();
		if (!newest()) throw new Stale();
		if (!resp.ok) throw new Error(answer || resp.statusText);
		return JSON.parse(answer);
	}
}

let last: { folder: string; feature: string; sidecar: Sidecar; sequence: number } | null = null;

/**
 * Saves `features` and tracks the one named `feature`; resolves with its sidecar, or rejects with `Stale` when
 * a newer call was made.
 */
export async function track(folder: string, features: Feature[], feature: string): Promise<Sidecar> {
	const mine = ++sequence;
	const sidecar = await postTrack({ folder, features, feature }, () => mine === sequence);
	last = { folder, feature, sidecar, sequence: mine };
	return sidecar;
}

/** The marking screen's sidecar for `feature` (no pad), while no newer track has been asked for since. */
export function lastSidecar(folder: string, feature: string): Sidecar | null {
	return last?.folder === folder && last.feature === feature && last.sequence === sequence ? last.sidecar : null;
}

/** `track` with `pad` (the CLI's `--pad`, none when absent); `Stale` when a newer call was made. */
export function trackPad(folder: string, features: Feature[], feature: string, pad?: string): Promise<Sidecar> {
	const mine = ++sequence;
	return postTrack({ folder, features, feature, ...(pad && { pad }) }, () => mine === sequence);
}

export interface ExportRequest {
	folder: string;
	/** The whole document, saved; `feature` names the one exported. */
	features: Feature[];
	feature: string;
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

/** Renders the clip. `onLine` sees each line the tracker reports; rejects with the server's message on failure. */
export async function exportClip(req: ExportRequest, onLine: (line: string) => void): Promise<Exported> {
	const resp = await fetch(`${currentConnection()}/api/stabilize/export`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(req)
	});
	if (!resp.ok || !resp.body) throw new Error((await resp.text()) || resp.statusText);
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

/** The clip's bytes, for the done state to show and download; `.json` is its sidecar. */
export const clipUrl = (path: string) => getImageSrc(path);

/** Finder opens on the instance's machine, so it is offered to a page on that same machine. */
export const canReveal = () => isLocalInstance(currentConnection() || location.origin);

/** Shows the clip in Finder on the machine the instance runs on. */
export async function revealClip(path: string): Promise<void> {
	const resp = await fetch(`${currentConnection()}/api/stabilize/reveal`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ path })
	});
	if (!resp.ok) throw new Error((await resp.text()) || resp.statusText);
}
