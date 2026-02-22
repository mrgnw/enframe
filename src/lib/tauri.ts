import { convertFileSrc } from '@tauri-apps/api/core';

export function getImageSrc(filePath: string): string {
	return convertFileSrc(filePath);
}

export async function getFilePathFromDrop(file: File): Promise<string | null> {
	// In Tauri, dropped files have a `path` property
	const path = (file as any).path;
	return path || null;
}
