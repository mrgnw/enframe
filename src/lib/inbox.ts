import { invoke } from '@tauri-apps/api/core';

export interface InboxFile {
	path: string;
	filename: string;
	size: number;
	modified: number;
}

export function scanInbox(directory: string): Promise<InboxFile[]> {
	return invoke<InboxFile[]>('scan_inbox', { directory });
}

const INBOX_DIR_KEY = 'enframe_inbox_dir';

export function getInboxDir(): string | null {
	return localStorage.getItem(INBOX_DIR_KEY);
}

export function setInboxDir(dir: string | null): void {
	if (dir) {
		localStorage.setItem(INBOX_DIR_KEY, dir);
	} else {
		localStorage.removeItem(INBOX_DIR_KEY);
	}
}
