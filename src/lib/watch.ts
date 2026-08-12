import { invoke } from './bridge';

export interface WatchFile {
	path: string;
	filename: string;
	size: number;
	modified: number;
}

export function scanWatchDir(directory: string): Promise<WatchFile[]> {
	return invoke<WatchFile[]>('scan_inbox', { directory });
}

const WATCH_DIR_KEY = 'enframe_watch_dir';

export function getWatchDir(): string | null {
	return localStorage.getItem(WATCH_DIR_KEY);
}

export function persistWatchDir(dir: string | null): void {
	if (dir) {
		localStorage.setItem(WATCH_DIR_KEY, dir);
	} else {
		localStorage.removeItem(WATCH_DIR_KEY);
	}
}
