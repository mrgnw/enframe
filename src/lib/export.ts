import { save } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import type { Photo, Frame } from './db';
import { getImageSrc } from './tauri';

export async function exportFrameAsJPEG(
	photo: Photo,
	frame: Frame,
	quality: number = 0.92
): Promise<string | null> {
	const img = new Image();
	img.src = getImageSrc(photo.file_path);
	
	await new Promise((resolve, reject) => {
		img.onload = resolve;
		img.onerror = reject;
	});

	// Create canvas with frame dimensions
	const canvas = document.createElement('canvas');
	const ctx = canvas.getContext('2d');
	if (!ctx) throw new Error('Could not get canvas context');

	// Calculate crop region in pixels
	const cropX = frame.x * img.naturalWidth;
	const cropY = frame.y * img.naturalHeight;
	const cropW = frame.width * img.naturalWidth;
	const cropH = frame.height * img.naturalHeight;

	canvas.width = cropW;
	canvas.height = cropH;

	// Draw the cropped region
	ctx.drawImage(
		img,
		cropX, cropY, cropW, cropH,  // source rectangle
		0, 0, cropW, cropH            // destination rectangle
	);

	// Convert to blob
	const blob = await new Promise<Blob | null>((resolve) => {
		canvas.toBlob(resolve, 'image/jpeg', quality);
	});

	if (!blob) throw new Error('Failed to create blob');

	// Convert blob to base64
	const reader = new FileReader();
	const base64 = await new Promise<string>((resolve, reject) => {
		reader.onloadend = () => resolve(reader.result as string);
		reader.onerror = reject;
		reader.readAsDataURL(blob);
	});

	// Prompt for save location
	const frameLabel = frame.label || `frame-${frame.id}`;
	const photoName = photo.title?.replace(/\s+/g, '-') || 'photo';
	const defaultPath = `${photoName}-${frameLabel}.jpg`;

	const savePath = await save({
		defaultPath,
		filters: [{
			name: 'JPEG',
			extensions: ['jpg', 'jpeg']
		}]
	});

	if (!savePath) return null;

	// Remove data URL prefix
	const base64Data = base64.split(',')[1];

	// Save via Tauri command
	await invoke('save_image', {
		path: savePath,
		data: base64Data
	});

	return savePath;
}

export async function exportAllFrames(
	photo: Photo,
	frames: Frame[],
	quality: number = 0.92,
	onProgress?: (current: number, total: number) => void
): Promise<string[]> {
	const paths: string[] = [];

	for (let i = 0; i < frames.length; i++) {
		if (onProgress) onProgress(i + 1, frames.length);
		
		const path = await exportFrameAsJPEG(photo, frames[i], quality);
		if (path) paths.push(path);
	}

	return paths;
}
