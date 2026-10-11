interface TauriInternals {
	convertFileSrc(filePath: string, protocol?: string): string;
}

// Window chrome, native dialogs and path-carrying drag-drop: a runtime capability, and
// deliberately not the same question as where the data lives.
export function hasNativeShell(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

function internals(): TauriInternals {
	return (window as unknown as { __TAURI_INTERNALS__: TauriInternals }).__TAURI_INTERNALS__;
}

const DEFAULT_PORT = 5911;
const CONNECTION_KEY = 'enframe.connection';
const KNOWN_KEY = 'enframe.connections';
const MAX_KNOWN = 8;

// The instance that owns the database, as a base URL. A browser is served by that instance
// and cannot reach another one across origins, so its base is the empty string. The native
// shell defaults to the loopback instance the Rust side probed for or spawned, and can be
// pointed at any instance the user names.
let base: string | null = null;

// Resolved once by the root layout, before anything renders, so the path-shaped helpers
// below can stay synchronous.
export async function resolveConnection(): Promise<string> {
	if (base !== null) return base;
	if (!hasNativeShell()) {
		base = '';
		return base;
	}
	const saved = stored(CONNECTION_KEY);
	if (saved) {
		base = saved;
		return base;
	}
	const { invoke: tauriInvoke } = await import('@tauri-apps/api/core');
	base = await tauriInvoke<string>('connection_url');
	return base;
}

function connection(): string {
	if (base === null) {
		throw new Error('connection used before resolveConnection() — the root layout awaits it');
	}
	return base;
}

export function currentConnection(): string {
	return connection();
}

const LOOPBACK = ['127.0.0.1', 'localhost', '::1', '[::1]'];

// The empty base is a browser served by its own instance, which has no filesystem to
// share whether or not that instance is on this machine.
export function isLocalInstance(url: string): boolean {
	if (url === '') return false;
	return LOOPBACK.includes(new URL(url).hostname);
}

// Whether this client reaches the same disk the connected instance does. Every path-shaped
// decision turns on this rather than on whether there is a window frame: a native shell
// pointed at another machine has no more access to that machine's files than a browser tab
// does, and must speak the server's vocabulary instead.
export function sharesFilesystem(): boolean {
	return hasNativeShell() && isLocalInstance(connection());
}

function stored(key: string): string | null {
	try {
		return localStorage.getItem(key);
	} catch {
		return null;
	}
}

// A bare host means this app's port, not the web's — `192.168.1.9` is the useful thing to
// be able to type. An explicit scheme is left alone, so a tunnel on 443 still works.
export function normalizeConnection(url: string): string {
	const trimmed = url.trim().replace(/\/+$/, '');
	if (!trimmed) return '';
	const hasScheme = /^https?:\/\//i.test(trimmed);
	const parsed = new URL(hasScheme ? trimmed : `http://${trimmed}`);
	if (!hasScheme && !parsed.port) parsed.port = String(DEFAULT_PORT);
	return `${parsed.protocol}//${parsed.host}`;
}

export function knownConnections(): string[] {
	try {
		const saved: unknown = JSON.parse(stored(KNOWN_KEY) ?? '[]');
		if (!Array.isArray(saved)) return [];
		return saved.filter((value): value is string => typeof value === 'string').slice(0, MAX_KNOWN);
	} catch {
		return [];
	}
}

function remember(url: string) {
	const next = [url, ...knownConnections().filter((known) => known !== url)].slice(0, MAX_KNOWN);
	try {
		localStorage.setItem(KNOWN_KEY, JSON.stringify(next));
		localStorage.setItem(CONNECTION_KEY, url);
	} catch {}
}

async function answers(candidate: string): Promise<boolean> {
	const response = await fetch(`${candidate}/sql/select`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ sql: 'SELECT 1 FROM photos LIMIT 1', values: [] })
	}).catch(() => null);
	return response?.ok ?? false;
}

// Probes before committing, so a typo cannot strand the app pointing at nothing. The
// caller reloads: every list on screen belongs to the instance we just left.
export async function connectTo(url: string): Promise<string> {
	const candidate = normalizeConnection(url);
	if (!candidate) throw new Error('enter an address like 192.168.1.9 or 100.64.0.7:5911');
	if (!(await answers(candidate))) {
		throw new Error(`no enframe instance answered at ${candidate}`);
	}
	remember(candidate);
	base = candidate;
	return candidate;
}

// Back to whatever this machine serves, forgetting the remembered target.
export async function connectToLocalInstance(): Promise<string> {
	try {
		localStorage.removeItem(CONNECTION_KEY);
	} catch {}
	base = null;
	return resolveConnection();
}

async function request(path: string, init?: RequestInit): Promise<Response> {
	const response = await fetch(`${connection()}${path}`, init);
	if (!response.ok) {
		const detail = await response.text();
		throw new Error(detail || `${response.status} ${response.statusText}`);
	}
	return response;
}

function postJson(path: string, body: unknown): Promise<Response> {
	return request(path, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(body)
	});
}

// On the machine that owns the data, commands run in-process and reach any path the user
// can. Connected elsewhere they go to the instance that owns the data, where the roots
// bound them — the paths they carry only mean anything there.
export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
	if (sharesFilesystem()) {
		const { invoke: tauriInvoke } = await import('@tauri-apps/api/core');
		return tauriInvoke<T>(command, args);
	}
	const response = await postJson(`/api/${command}`, args ?? {});
	return response.json() as Promise<T>;
}

export interface Db {
	select<T>(sql: string, values?: unknown[]): Promise<T>;
	execute(sql: string, values?: unknown[]): Promise<{ lastInsertId?: number }>;
}

const httpDb: Db = {
	async select<T>(sql: string, values?: unknown[]): Promise<T> {
		const response = await postJson('/sql/select', { sql, values: values ?? [] });
		return response.json() as Promise<T>;
	},
	async execute(sql: string, values?: unknown[]): Promise<{ lastInsertId?: number }> {
		const response = await postJson('/sql/execute', { sql, values: values ?? [] });
		return response.json() as Promise<{ lastInsertId?: number }>;
	}
};

// One writer: whoever owns the database serves it, and every frontend is a client of it.
export async function getDb(): Promise<Db> {
	return httpDb;
}

// ponytail: one probe stands in for the whole backend, the library UI needs all of it or none.
export function libraryReachable(): Promise<boolean> {
	return answers(connection());
}

export function getImageSrc(path: string): string {
	if (sharesFilesystem()) return internals().convertFileSrc(path);
	return `${connection()}/img/${encodeURIComponent(path)}`;
}

export async function readTextFile(path: string): Promise<string> {
	if (sharesFilesystem()) {
		const { readTextFile: tauriReadTextFile } = await import('@tauri-apps/plugin-fs');
		return tauriReadTextFile(path);
	}
	const response = await request(`/api/read_text?path=${encodeURIComponent(path)}`);
	return response.text();
}

function toBase64(bytes: Uint8Array): string {
	const CHUNK = 0x8000;
	let binary = '';
	for (let i = 0; i < bytes.length; i += CHUNK) {
		binary += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
	}
	return btoa(binary);
}

export async function writeFile(path: string, bytes: Uint8Array): Promise<void> {
	if (sharesFilesystem()) {
		const { writeFile: tauriWriteFile } = await import('@tauri-apps/plugin-fs');
		await tauriWriteFile(path, bytes);
		return;
	}
	await postJson('/api/save_image', { path, data: toBase64(bytes) });
}

export async function stat(path: string): Promise<{ isDirectory: boolean }> {
	if (sharesFilesystem()) {
		const { stat: tauriStat } = await import('@tauri-apps/plugin-fs');
		return tauriStat(path);
	}
	const response = await request(`/api/stat?path=${encodeURIComponent(path)}`);
	return response.json() as Promise<{ isDirectory: boolean }>;
}

export interface LsEntry {
	name: string;
	path: string;
	isDirectory: boolean;
	isImage: boolean;
}

export interface Listing {
	path: string;
	parent: string | null;
	entries: LsEntry[];
}

export async function listDirectory(dir?: string): Promise<Listing> {
	const query = dir ? `?dir=${encodeURIComponent(dir)}` : '';
	const response = await request(`/api/ls${query}`);
	return response.json() as Promise<Listing>;
}

// Picking a photo means picking one the instance can open, so the server-side picker is
// the right dialog whenever the files live on another machine — the native one would
// return paths that mean nothing there.
export async function pickDirectory(title: string): Promise<string | null> {
	if (!sharesFilesystem()) {
		const { browserPick } = await import('./picker');
		const selected = await browserPick({ title, directory: true });
		return selected?.[0] ?? null;
	}
	const { open } = await import('@tauri-apps/plugin-dialog');
	const selected = await open({ directory: true, multiple: false, title });
	return typeof selected === 'string' ? selected : null;
}

export async function pickFiles(options: {
	title?: string;
	multiple?: boolean;
	extensions?: string[];
}): Promise<string[] | null> {
	if (!sharesFilesystem()) {
		const { browserPick } = await import('./picker');
		return browserPick(options);
	}
	const { open } = await import('@tauri-apps/plugin-dialog');
	const selected = await open({
		title: options.title,
		multiple: options.multiple ?? false,
		filters: options.extensions ? [{ name: 'Images', extensions: options.extensions }] : undefined
	});
	if (!selected) return null;
	return Array.isArray(selected) ? selected : [selected];
}

export type DragDropPayload =
	| { type: 'enter'; paths: string[]; position: { x: number; y: number } }
	| { type: 'over'; position: { x: number; y: number } }
	| { type: 'drop'; paths: string[]; position: { x: number; y: number } }
	| { type: 'leave' };

const DETECT_MAX_SIDE = 1280;

// A tab that opened a photo from disk holds pixels but no path the instance can read.
// Send what the tab shows, oriented by the browser and downscaled to what the detector
// uses anyway, so the points land where the user sees the faces and nothing is stored.
export async function detectFacesInImage<T>(src: string): Promise<T> {
	const image = new Image();
	image.src = src;
	await image.decode();
	const scale = Math.min(1, DETECT_MAX_SIDE / Math.max(image.naturalWidth, image.naturalHeight));
	const canvas = document.createElement('canvas');
	canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
	canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
	const context = canvas.getContext('2d');
	if (!context) throw new Error('no 2d canvas context');
	context.drawImage(image, 0, 0, canvas.width, canvas.height);
	const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/jpeg', 0.92));
	if (!blob) throw new Error('could not encode the image');
	const response = await request('/api/detect_faces_bytes', { method: 'POST', body: blob });
	return response.json() as Promise<T>;
}

// Browser drops carry bytes, not paths, so each file lands in the server inbox and
// the resulting absolute path stands in for the one Tauri would have given us.
async function uploadDropped(file: File): Promise<string> {
	const response = await request('/api/upload', {
		method: 'POST',
		headers: { 'x-filename': encodeURIComponent(file.name) },
		body: file
	});
	const uploaded = (await response.json()) as { path: string };
	return uploaded.path;
}

function browserDragDrop(handler: (payload: DragDropPayload) => void): () => void {
	let depth = 0;

	const enter = (event: DragEvent) => {
		event.preventDefault();
		depth += 1;
		if (depth === 1) {
			handler({ type: 'enter', paths: [], position: { x: event.clientX, y: event.clientY } });
		}
	};
	const over = (event: DragEvent) => {
		event.preventDefault();
		handler({ type: 'over', position: { x: event.clientX, y: event.clientY } });
	};
	const leave = () => {
		depth = Math.max(0, depth - 1);
		if (depth === 0) handler({ type: 'leave' });
	};
	const drop = async (event: DragEvent) => {
		event.preventDefault();
		depth = 0;
		const files = Array.from(event.dataTransfer?.files ?? []);
		const position = { x: event.clientX, y: event.clientY };
		const paths: string[] = [];
		for (const file of files) {
			paths.push(await uploadDropped(file));
		}
		handler({ type: 'drop', paths, position });
	};

	window.addEventListener('dragenter', enter);
	window.addEventListener('dragover', over);
	window.addEventListener('dragleave', leave);
	window.addEventListener('drop', drop);

	return () => {
		window.removeEventListener('dragenter', enter);
		window.removeEventListener('dragover', over);
		window.removeEventListener('dragleave', leave);
		window.removeEventListener('drop', drop);
	};
}

// A native drop hands us paths on this machine, which the instance that owns the data
// cannot see when it lives elsewhere. Same ending as a browser drop: send the bytes and
// let the server name the file.
async function uploadLocal(path: string): Promise<string> {
	const { readFile } = await import('@tauri-apps/plugin-fs');
	const bytes = await readFile(path);
	const name = path.split('/').pop() || 'dropped';
	const response = await request('/api/upload', {
		method: 'POST',
		headers: { 'x-filename': encodeURIComponent(name) },
		body: new Blob([bytes as BlobPart])
	});
	const uploaded = (await response.json()) as { path: string };
	return uploaded.path;
}

export async function onDragDrop(handler: (payload: DragDropPayload) => void): Promise<() => void> {
	if (!hasNativeShell()) return browserDragDrop(handler);
	const { getCurrentWebview } = await import('@tauri-apps/api/webview');
	return getCurrentWebview().onDragDropEvent((event) => {
		const payload = event.payload as DragDropPayload;
		if (payload.type !== 'drop' || sharesFilesystem()) {
			handler(payload);
			return;
		}
		void Promise.all(payload.paths.map(uploadLocal)).then((paths) => handler({ ...payload, paths }));
	});
}
