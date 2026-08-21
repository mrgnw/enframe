import { invoke } from './bridge';

export interface FolderFile {
	path: string;
	filename: string;
	size: number;
	modified: number;
}

export function scanFolder(directory: string): Promise<FolderFile[]> {
	return invoke<FolderFile[]>('scan_inbox', { directory });
}

const FOLDERS_KEY = 'enframe.folders';
const OLD_WATCH_KEY = 'enframe_watch_dir';

export function getFolders(): string[] {
	if (typeof localStorage === 'undefined') return [];
	let folders: string[] = [];
	try {
		folders = JSON.parse(localStorage.getItem(FOLDERS_KEY) ?? '[]');
	} catch {}
	const watched = localStorage.getItem(OLD_WATCH_KEY);
	if (watched) {
		localStorage.removeItem(OLD_WATCH_KEY);
		if (!folders.includes(watched)) {
			folders = [...folders, watched];
			persistFolders(folders);
		}
	}
	return folders;
}

export function persistFolders(folders: string[]): void {
	localStorage.setItem(FOLDERS_KEY, JSON.stringify(folders));
}
