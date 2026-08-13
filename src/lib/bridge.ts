interface TauriInternals {
	convertFileSrc(filePath: string, protocol?: string): string;
}

export function isTauri(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

function internals(): TauriInternals {
	return (window as unknown as { __TAURI_INTERNALS__: TauriInternals }).__TAURI_INTERNALS__;
}

async function request(path: string, init?: RequestInit): Promise<Response> {
	const response = await fetch(path, init);
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

export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
	if (isTauri()) {
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

let dbPromise: Promise<Db> | null = null;

async function loadDb(): Promise<Db> {
	if (!isTauri()) return httpDb;
	const { default: Database } = await import('@tauri-apps/plugin-sql');
	return Database.load('sqlite:enframe.db');
}

export function getDb(): Promise<Db> {
	if (!dbPromise) {
		dbPromise = loadDb().catch((err) => {
			console.error('[bridge] database load FAILED:', err);
			dbPromise = null;
			throw err;
		});
	}
	return dbPromise;
}

export function getImageSrc(path: string): string {
	if (isTauri()) return internals().convertFileSrc(path);
	return `/img/${encodeURIComponent(path)}`;
}

export async function readTextFile(path: string): Promise<string> {
	if (isTauri()) {
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
	if (isTauri()) {
		const { writeFile: tauriWriteFile } = await import('@tauri-apps/plugin-fs');
		await tauriWriteFile(path, bytes);
		return;
	}
	await postJson('/api/save_image', { path, data: toBase64(bytes) });
}

export async function stat(path: string): Promise<{ isDirectory: boolean }> {
	if (isTauri()) {
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

export async function pickDirectory(title: string): Promise<string | null> {
	if (!isTauri()) {
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
	if (!isTauri()) {
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

export async function onDragDrop(handler: (payload: DragDropPayload) => void): Promise<() => void> {
	if (!isTauri()) return browserDragDrop(handler);
	const { getCurrentWebview } = await import('@tauri-apps/api/webview');
	return getCurrentWebview().onDragDropEvent((event) => handler(event.payload));
}
