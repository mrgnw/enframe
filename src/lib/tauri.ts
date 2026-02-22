import { convertFileSrc } from '@tauri-apps/api/core';

export function getImageSrc(filePath: string): string {
	return convertFileSrc(filePath);
}
