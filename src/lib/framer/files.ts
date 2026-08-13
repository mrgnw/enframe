import { RAW_EXTENSIONS } from './raw';

interface PickerAccept {
	description?: string;
	accept: Record<string, string[]>;
}

interface OpenPickerOptions {
	multiple?: boolean;
	excludeAcceptAllOption?: boolean;
	types?: PickerAccept[];
}

interface SavePickerOptions {
	suggestedName?: string;
	types?: PickerAccept[];
}

declare global {
	interface Window {
		showOpenFilePicker?: (options?: OpenPickerOptions) => Promise<FileSystemFileHandle[]>;
		showSaveFilePicker?: (options?: SavePickerOptions) => Promise<FileSystemFileHandle>;
		showDirectoryPicker?: (options?: { mode?: 'read' | 'readwrite' }) => Promise<FileSystemDirectoryHandle>;
	}
}

// Chromium ships the pickers; Safari and Firefox ship the handle types but not the
// pickers, so detect the picker rather than the handle. Detected once, at module load.
export const fileSystemAccess: boolean =
	typeof window !== 'undefined' &&
	typeof window.showOpenFilePicker === 'function' &&
	typeof window.showDirectoryPicker === 'function';

const RAW_ACCEPT = RAW_EXTENSIONS.map((extension) => `.${extension}`);

export const IMAGE_TYPES: PickerAccept[] = [
	{
		description: 'Image and sidecar',
		accept: {
			'image/*': ['.jpg', '.jpeg', '.png', '.webp', '.avif', '.gif', '.tif', '.tiff'],
			'application/json': ['.json']
		}
	},
	{
		// Camera RAW has no image/* MIME the pickers agree on, so it gets its own entry.
		description: 'Camera RAW',
		accept: { 'application/octet-stream': RAW_ACCEPT }
	}
];

export interface PickedFile {
	file: File;
	// Present only where the File System Access API is: it lets a later save overwrite
	// the same file without asking again.
	handle: FileSystemFileHandle | null;
}

function isAbort(error: unknown): boolean {
	return error instanceof DOMException && error.name === 'AbortError';
}

// The fallback input never reports a cancel on older browsers, so the promise it
// returns can stay pending until the element is collected. Nothing waits on it but
// the click handler, which is fine.
function pickWithInput(): Promise<PickedFile[]> {
	return new Promise((resolve) => {
		const input = document.createElement('input');
		input.type = 'file';
		input.multiple = true;
		input.accept = `image/*,application/json,.json,${RAW_ACCEPT.join(',')}`;
		input.addEventListener('change', () => {
			resolve(Array.from(input.files ?? []).map((file) => ({ file, handle: null })));
		});
		input.addEventListener('cancel', () => resolve([]));
		input.click();
	});
}

export async function pickImageFiles(): Promise<PickedFile[]> {
	if (!fileSystemAccess || !window.showOpenFilePicker) return pickWithInput();
	try {
		const handles = await window.showOpenFilePicker({ multiple: true, types: IMAGE_TYPES });
		return Promise.all(handles.map(async (handle) => ({ file: await handle.getFile(), handle })));
	} catch (error) {
		if (isAbort(error)) return [];
		throw error;
	}
}

// ponytail: the handle lives for the page session only. Persisting it through
// IndexedDB and re-requesting permission is the upgrade if reopening a folder
// across reloads is ever worth the prompt.
export async function pickOutputDirectory(): Promise<FileSystemDirectoryHandle | null> {
	if (!fileSystemAccess || !window.showDirectoryPicker) return null;
	try {
		return await window.showDirectoryPicker({ mode: 'readwrite' });
	} catch (error) {
		if (isAbort(error)) return null;
		throw error;
	}
}

export async function writeInto(dir: FileSystemDirectoryHandle, name: string, blob: Blob): Promise<void> {
	const handle = await dir.getFileHandle(name, { create: true });
	await writeHandle(handle, blob);
}

export async function writeHandle(handle: FileSystemFileHandle, blob: Blob): Promise<void> {
	const stream = await handle.createWritable();
	await stream.write(blob);
	await stream.close();
}

export function download(name: string, blob: Blob): void {
	const url = URL.createObjectURL(blob);
	const anchor = document.createElement('a');
	anchor.href = url;
	anchor.download = name;
	anchor.click();
	URL.revokeObjectURL(url);
}

// Writes wherever the browser will let us: an already-granted handle, a fresh save
// dialog whose handle the caller should keep, or the downloads folder.
export async function saveAs(
	name: string,
	blob: Blob,
	handle: FileSystemFileHandle | null
): Promise<FileSystemFileHandle | null> {
	if (handle) {
		await writeHandle(handle, blob);
		return handle;
	}
	if (!fileSystemAccess || !window.showSaveFilePicker) {
		download(name, blob);
		return null;
	}
	try {
		const picked = await window.showSaveFilePicker({ suggestedName: name });
		await writeHandle(picked, blob);
		return picked;
	} catch (error) {
		if (isAbort(error)) return null;
		throw error;
	}
}
