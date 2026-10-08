// The thin backend of the MVP: turns a marks document into a pinframe CLI call, runs it, and hands back
// the sidecar. One run per folder at a time; a newer request kills the one in flight. At E2 this becomes
// an axum route in enframe-serve, so it stays as small as the CLI's own flags.

import { spawn, type ChildProcess } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readdir, readFile, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, dirname, extname, isAbsolute, join, resolve } from 'node:path';

/** The repository root: `pnpm dev` and `node build` both run from `web/`. Relative paths resolve here. */
export const ROOT = basename(process.cwd()) === 'web' ? dirname(process.cwd()) : process.cwd();

/** The pinframe binary, `PINFRAME_BIN` or the release build. */
export const BIN = resolve(ROOT, process.env.PINFRAME_BIN ?? 'rust/target/release/pinframe');

const PHOTO_EXTENSIONS = new Set(['.avif', '.heic', '.heif', '.jxl', '.jpg', '.jpeg', '.png']);

export type Point = [number, number];

/**
 * The document the UI edits, saved beside the folder as `<folder>.pin.json`. Coordinates are pixels of
 * the photo as displayed (after EXIF orientation), exactly what the CLI takes, so the file reproduces the
 * CLI call: `points` is `--points` on the first photo, each `at` entry is `--at FILE=points[@zoom]`, `skip`
 * is `--skip FILE`, `order` is `--order`.
 */
export interface Marks {
	points: Point[];
	at?: Record<string, { points: Point[]; zoom?: number }>;
	skip?: string[];
	order?: 'time' | 'name';
}

export interface ExportOptions {
	/** The clip's path; its extension picks the format. Default `<folder>.avif`, beside the folder. */
	out?: string;
	pad?: string;
	fps?: number;
	/** A directory for the aligned PNG frames (`--frames`). */
	frames?: string;
	vf?: string;
}

export interface SidecarPhoto {
	file: string;
	x: number;
	y: number;
	w: number;
	h: number;
	scale: number;
	source: 'tracked' | 'mark' | 'lost' | 'skipped';
	points: Point[];
	/** Added from the run's log: the photo's oriented size and the tracker's note (`38 features`, `your mark`, ...). */
	width?: number;
	height?: number;
	note?: string;
}

export interface Sidecar {
	run: string;
	clip: string | null;
	photos: SidecarPhoto[];
}

/**
 * `IMG_4669.avif  2048x1536  anchor 1040,725  scale 2.50  38 features`: what the CLI prints per photo. The
 * columns are two spaces apart, so a name may hold single spaces (`Photo 1.jpg`).
 */
const PHOTO_LINE = /^(.+?)  (\d+)x(\d+)  anchor \S+\s+scale \S+\s+(.*)$/;

/** Adds each photo's size and note from the run's output to the sidecar, which carries neither. */
export function annotate(sidecar: Sidecar, stdout: string): Sidecar {
	const rows = new Map<string, { width: number; height: number; note: string }>();
	for (const line of stdout.split('\n')) {
		const m = PHOTO_LINE.exec(line);
		if (m) rows.set(m[1], { width: Number(m[2]), height: Number(m[3]), note: m[4] });
	}
	for (const photo of sidecar.photos) {
		Object.assign(photo, rows.get(photo.file));
	}
	return sidecar;
}

export class PinframeError extends Error {
	constructor(
		message: string,
		public status = 400
	) {
		super(message);
	}
}

function point(p: unknown, what: string): Point {
	if (!Array.isArray(p) || p.length !== 2 || !p.every((v) => typeof v === 'number' && Number.isFinite(v))) {
		throw new PinframeError(`${what}: a point is [x, y] in pixels`);
	}
	return [p[0], p[1]];
}

function points(list: unknown, what: string): Point[] {
	if (!Array.isArray(list) || list.length === 0) {
		throw new PinframeError(`${what}: needs at least one point`);
	}
	return list.map((p) => point(p, what));
}

/** Validates a request body's `marks`; the shape mirrors the CLI, so a bad one gets the CLI's wording. */
export function parseMarks(raw: unknown): Marks {
	if (typeof raw !== 'object' || raw === null) {
		throw new PinframeError('marks must be an object');
	}
	const m = raw as Record<string, unknown>;
	const marks: Marks = { points: points(m.points, 'points') };
	if (m.at !== undefined) {
		if (typeof m.at !== 'object' || m.at === null) {
			throw new PinframeError('at must map a photo name to {points, zoom?}');
		}
		marks.at = {};
		for (const [file, mark] of Object.entries(m.at as Record<string, unknown>)) {
			if (typeof mark !== 'object' || mark === null) {
				throw new PinframeError(`at ${file}: needs {points, zoom?}`);
			}
			const { points: p, zoom } = mark as Record<string, unknown>;
			if (zoom !== undefined && (typeof zoom !== 'number' || !(zoom > 0))) {
				throw new PinframeError(`at ${file}: zoom must be a positive number`);
			}
			marks.at[file] = { points: points(p, `at ${file}`), ...(zoom !== undefined && { zoom }) };
		}
	}
	if (m.skip !== undefined) {
		if (!Array.isArray(m.skip) || !m.skip.every((s) => typeof s === 'string')) {
			throw new PinframeError('skip must be a list of photo names');
		}
		marks.skip = m.skip;
	}
	if (m.order !== undefined) {
		if (m.order !== 'time' && m.order !== 'name') {
			throw new PinframeError('order must be time or name');
		}
		marks.order = m.order;
	}
	return marks;
}

/** A request's `pad`, the CLI's `--pad` value: transparent, black, white or #rrggbb. Absent means no padding. */
export function parsePad(raw: unknown): string | undefined {
	if (raw === undefined || raw === null || raw === 'none') return undefined;
	if (typeof raw !== 'string' || !/^(transparent|black|white|#[0-9a-fA-F]{6})$/.test(raw)) {
		throw new PinframeError('pad must be transparent, black, white or #rrggbb');
	}
	return raw;
}

const formatPoints = (list: Point[]) => list.map(([x, y]) => `${x},${y}`).join('/');

/** The CLI flags for a document, without the photos and `--out`. */
export function cliArgs(marks: Marks): string[] {
	const args = ['--points', formatPoints(marks.points)];
	for (const [file, mark] of Object.entries(marks.at ?? {})) {
		const zoom = mark.zoom === undefined ? '' : `@${mark.zoom}`;
		args.push('--at', `${file}=${formatPoints(mark.points)}${zoom}`);
	}
	for (const file of marks.skip ?? []) {
		args.push('--skip', file);
	}
	if (marks.order) {
		args.push('--order', marks.order);
	}
	return args;
}

/** The folder's photos by name, as absolute paths. pinframe orders them by capture time itself. */
export async function listPhotos(folder: string): Promise<string[]> {
	let entries;
	try {
		entries = await readdir(folder, { withFileTypes: true });
	} catch (e) {
		throw new PinframeError(`${folder}: ${(e as Error).message}`, 404);
	}
	const photos = entries
		.filter((e) => e.isFile() && !e.name.startsWith('.') && PHOTO_EXTENSIONS.has(extname(e.name).toLowerCase()))
		.map((e) => join(folder, e.name))
		.sort();
	if (photos.length < 2) {
		throw new PinframeError(`${folder}: need at least two photos (AVIF, HEIC, JPEG XL, JPEG or PNG)`);
	}
	return photos;
}

/** A folder from a request: absolute, or relative to the repository root. */
export function resolveFolder(raw: unknown): string {
	if (typeof raw !== 'string' || raw === '') {
		throw new PinframeError('folder must be a path');
	}
	return resolve(ROOT, raw);
}

/** The document beside the folder: `samples/train clouds` keeps `samples/train clouds.pin.json`. */
export const documentPath = (folder: string) => `${folder}.pin.json`;

/** The saved document's marks, or null when the folder has none yet. */
export async function loadDocument(folder: string): Promise<Marks | null> {
	let text;
	try {
		text = await readFile(documentPath(folder), 'utf8');
	} catch {
		return null;
	}
	return parseMarks((JSON.parse(text) as { marks: unknown }).marks);
}

/** A photo named in a request: a bare file name inside the folder, never a path. */
export function resolvePhoto(folder: string, file: unknown): string {
	if (typeof file !== 'string' || file === '' || file !== basename(file) || file.startsWith('.')) {
		throw new PinframeError('file must be a photo name in the folder');
	}
	if (!PHOTO_EXTENSIONS.has(extname(file).toLowerCase())) {
		throw new PinframeError(`${file}: not a photo (AVIF, HEIC, JPEG XL, JPEG or PNG)`);
	}
	return join(folder, file);
}

export const PHOTO_TYPES: Record<string, string> = {
	'.avif': 'image/avif',
	'.heic': 'image/heic',
	'.heif': 'image/heif',
	'.jxl': 'image/jxl',
	'.jpg': 'image/jpeg',
	'.jpeg': 'image/jpeg',
	'.png': 'image/png'
};

/** What `/api/clip` serves: the formats an export writes, and the sidecar `.json` beside one. */
export const CLIP_TYPES: Record<string, string> = {
	'.avif': 'image/avif',
	'.mp4': 'video/mp4',
	'.mov': 'video/quicktime',
	'.webm': 'video/webm',
	'.json': 'application/json'
};

/**
 * A clip named in a request: a path with a clip extension, or a `.json` that has a clip of that name
 * beside it. The server may read any path (see the README), but a route that serves bytes back to a browser
 * is held to what an export writes, so `/etc/passwd`, `package.json` and the `.pin.json` documents stay out.
 */
export async function resolveClip(raw: unknown): Promise<string> {
	if (typeof raw !== 'string' || raw === '') {
		throw new PinframeError('path must be a clip');
	}
	const path = isAbsolute(raw) ? raw : resolve(ROOT, raw);
	const ext = extname(path).toLowerCase();
	if (!(ext in CLIP_TYPES)) {
		throw new PinframeError(`${raw}: not a clip (AVIF, MP4, MOV or WebM) or its sidecar`);
	}
	if (ext === '.json') {
		const stem = path.slice(0, -ext.length);
		const beside = await Promise.all(
			Object.keys(CLIP_TYPES)
				.filter((e) => e !== '.json')
				.map((e) => stat(stem + e).then((i) => i.isFile(), () => false))
		);
		if (!beside.some(Boolean)) {
			throw new PinframeError(`${raw}: not a clip's sidecar`);
		}
	}
	return path;
}

/** Whether the server can show a file in Finder: only on the Mac. */
export const CAN_REVEAL = process.platform === 'darwin';

/** The last write queued per folder: writes chain on it, so two requests never interleave their `writeFile`s. */
const saving = new Map<string, Promise<void>>();

export function saveDocument(folder: string, marks: Marks, photos: string[]): Promise<void> {
	const doc = { version: 1, folder, marks, photos: photos.map((p) => basename(p)) };
	const text = JSON.stringify(doc, null, '\t') + '\n';
	const write = (saving.get(folder) ?? Promise.resolve())
		.catch(() => {})
		.then(() => writeFile(documentPath(folder), text));
	saving.set(folder, write);
	const done = () => {
		if (saving.get(folder) === write) saving.delete(folder);
	};
	write.then(done, done);
	return write;
}

// ---- running pinframe ----

export interface Run {
	id: number;
	folder: string;
	child: ChildProcess;
	/** `track` or the export's clip path. */
	what: string;
}

let nextId = 1;
const running = new Map<string, Run>();

/** The run in flight for a folder, if any. */
export const currentRun = (folder: string) => running.get(folder);

export class Superseded extends Error {
	constructor() {
		super('superseded by a newer run for this folder');
	}
}

/**
 * A folder's tickets: a request takes the next number the moment it arrives, before any await, so arrival
 * order is settled before the awaits can reorder anything. Only the newest ticket may save or spawn.
 */
const tickets = new Map<string, number>();

function claim(folder: string): number {
	const ticket = (tickets.get(folder) ?? 0) + 1;
	tickets.set(folder, ticket);
	return ticket;
}

/** Throws `Superseded` when a newer request for the folder has arrived since `ticket` was claimed. */
function ensureNewest(folder: string, ticket: number): void {
	if (tickets.get(folder) !== ticket) throw new Superseded();
}

interface Finished {
	code: number | null;
	signal: NodeJS.Signals | null;
	stdout: string;
	stderr: string;
}

/**
 * Starts pinframe for a folder, killing the run already in flight for it. `onLine` sees each output line
 * as it arrives. Resolves when the child exits; rejects with `Superseded` if a newer run replaced it.
 */
export function start(
	folder: string,
	what: string,
	args: string[],
	onLine?: (stream: 'stdout' | 'stderr', line: string) => void
): { run: Run; finished: Promise<Finished> } {
	const previous = running.get(folder);
	if (previous) {
		previous.child.kill('SIGTERM');
	}
	const child = spawn(BIN, args, { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
	const run: Run = { id: nextId++, folder, child, what };
	running.set(folder, run);
	const stdout: string[] = [];
	const stderr: string[] = [];
	const collect = (stream: 'stdout' | 'stderr', into: string[]) => {
		let pending = '';
		child[stream]!.setEncoding('utf8').on('data', (chunk: string) => {
			pending += chunk;
			const lines = pending.split('\n');
			pending = lines.pop() ?? '';
			for (const line of lines) {
				into.push(line);
				onLine?.(stream, line);
			}
		});
		child[stream]!.on('end', () => {
			if (pending) {
				into.push(pending);
				onLine?.(stream, pending);
			}
		});
	};
	collect('stdout', stdout);
	collect('stderr', stderr);
	const finished = new Promise<Finished>((resolvePromise, reject) => {
		child.on('error', (e) => {
			if (running.get(folder) === run) running.delete(folder);
			reject(new PinframeError(`${BIN}: ${e.message}`, 500));
		});
		child.on('close', (code, signal) => {
			const superseded = running.get(folder) !== run;
			if (!superseded) running.delete(folder);
			if (superseded) {
				reject(new Superseded());
			} else {
				resolvePromise({ code, signal, stdout: stdout.join('\n'), stderr: stderr.join('\n') });
			}
		});
	});
	return { run, finished };
}

function failure(what: string, finished: Finished): PinframeError {
	const detail = finished.stderr.trim().split('\n').pop() || `exit ${finished.code ?? finished.signal}`;
	return new PinframeError(`${what}: ${detail}`, 500);
}

/** Where a folder's JSON-only sidecar goes: a temp directory per folder, never beside the photos. */
async function trackOutput(folder: string): Promise<string> {
	const dir = join(tmpdir(), 'pinframe-web', createHash('sha1').update(folder).digest('hex').slice(0, 16));
	await mkdir(dir, { recursive: true });
	return join(dir, 'track.json');
}

/**
 * Tracks a folder with the document's marks, JSON-only with the feature cache, and returns the sidecar.
 * `pad` is `--pad`: the crop's rects depend on whether there is padding (they overhang the photos), not on
 * its colour.
 */
export async function track(folder: string, marks: Marks, pad?: string): Promise<Sidecar> {
	const ticket = claim(folder);
	const photos = await listPhotos(folder);
	ensureNewest(folder, ticket);
	await saveDocument(folder, marks, photos);
	const out = await trackOutput(folder);
	ensureNewest(folder, ticket);
	const args = [...photos, ...cliArgs(marks), '--out', out];
	if (pad) args.push('--pad', pad);
	const { finished } = start(folder, 'track', args);
	const result = await finished;
	if (result.code !== 0) {
		throw failure('track', result);
	}
	try {
		return annotate(JSON.parse(await readFile(out, 'utf8')) as Sidecar, result.stdout);
	} catch (e) {
		// A newer run may be rewriting the shared sidecar file: this answer is stale either way.
		ensureNewest(folder, ticket);
		throw e;
	}
}

/** A photo's sidecar partner: `clip.avif` has `clip.json` beside it. */
const sidecarOf = (out: string) => out.replace(/\.[^./]+$/, '') + '.json';

/** Same folder, compared without case: the Mac's file system ignores it. */
const sameFolder = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** The clip path for an export, refused inside the folder itself: the next run would read it as a photo. */
export function resolveOut(folder: string, out: unknown): string {
	if (out === undefined) return `${folder}.avif`;
	if (typeof out !== 'string' || out === '') {
		throw new PinframeError('out must be a path');
	}
	const path = isAbsolute(out) ? out : resolve(ROOT, out);
	if (sameFolder(dirname(path), folder)) {
		throw new PinframeError(`out ${out}: the clip cannot be written inside the photo folder`);
	}
	return path;
}

/** The `--frames` directory, refused as the folder itself for the same reason: its PNGs would become photos. */
function resolveFrames(folder: string, frames: string): string {
	const path = isAbsolute(frames) ? frames : resolve(ROOT, frames);
	if (sameFolder(path, folder)) {
		throw new PinframeError(`frames ${frames}: the frames cannot be written inside the photo folder`);
	}
	return path;
}

/**
 * Renders the clip. `onLine` streams pinframe's progress; the result is the clip's path and its sidecar.
 */
export async function exportClip(
	folder: string,
	marks: Marks,
	options: ExportOptions,
	onLine: (stream: 'stdout' | 'stderr', line: string) => void
): Promise<{ out: string; sidecar: Sidecar }> {
	// Refused paths answer before a ticket is taken, so a bad request never supersedes a good one.
	const out = resolveOut(folder, options.out);
	const frames = options.frames ? resolveFrames(folder, options.frames) : undefined;
	const ticket = claim(folder);
	const photos = await listPhotos(folder);
	ensureNewest(folder, ticket);
	await saveDocument(folder, marks, photos);
	ensureNewest(folder, ticket);
	const args = [...photos, ...cliArgs(marks), '--out', out];
	if (options.pad) args.push('--pad', options.pad);
	if (options.fps !== undefined) args.push('--fps', String(options.fps));
	if (frames) args.push('--frames', frames);
	if (options.vf) args.push('--vf', options.vf);
	const { finished } = start(folder, out, args, onLine);
	const result = await finished;
	if (result.code !== 0) {
		throw failure('export', result);
	}
	const sidecar = annotate(JSON.parse(await readFile(sidecarOf(out), 'utf8')) as Sidecar, result.stdout);
	return { out, sidecar };
}
