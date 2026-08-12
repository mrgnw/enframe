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

export async function pickDirectory(title: string): Promise<string | null> {
	if (!isTauri()) return null;
	const { open } = await import('@tauri-apps/plugin-dialog');
	const selected = await open({ directory: true, multiple: false, title });
	return typeof selected === 'string' ? selected : null;
}

export async function pickFiles(options: {
	title?: string;
	multiple?: boolean;
	extensions?: string[];
}): Promise<string[] | null> {
	if (!isTauri()) return null;
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

export async function onDragDrop(handler: (payload: DragDropPayload) => void): Promise<() => void> {
	if (!isTauri()) return () => {};
	const { getCurrentWebview } = await import('@tauri-apps/api/webview');
	return getCurrentWebview().onDragDropEvent((event) => handler(event.payload));
}
