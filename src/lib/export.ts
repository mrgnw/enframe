import { open } from '@tauri-apps/plugin-dialog';
import { writeFile } from '@tauri-apps/plugin-fs';
import type { FocalPoint, Frame, Photo } from './db';
import { getImageSrc } from './tauri';

async function cropFrameToBlob(photo: Photo, frame: Frame, quality: number = 0.92): Promise<Blob> {
	const img = new Image();
	img.src = getImageSrc(photo.file_path);

	await new Promise((resolve, reject) => {
		img.onload = resolve;
		img.onerror = reject;
	});

	const canvas = document.createElement('canvas');
	const ctx = canvas.getContext('2d');
	if (!ctx) throw new Error('Could not get canvas context');

	const cropX = frame.x * img.naturalWidth;
	const cropY = frame.y * img.naturalHeight;
	const cropW = frame.width * img.naturalWidth;
	const cropH = frame.height * img.naturalHeight;

	canvas.width = cropW;
	canvas.height = cropH;

	ctx.drawImage(img, cropX, cropY, cropW, cropH, 0, 0, cropW, cropH);

	const blob = await new Promise<Blob | null>((resolve) => {
		canvas.toBlob(resolve, 'image/jpeg', quality);
	});

	if (!blob) throw new Error('Failed to create blob');
	return blob;
}

function generateFrameFilename(photo: Photo, frame: Frame, index: number): string {
	const basename = photo.title?.replace(/\.[^/.]+$/, '').replace(/\s+/g, '-') || 'photo';
	const ratio = (frame.width / frame.height).toFixed(2);
	const orientation = frame.width > frame.height ? 'l' : frame.width < frame.height ? 'p' : 's';
	return `${basename}.${orientation}.${index + 1}.${ratio}.jpg`;
}

export async function exportFramesToDirectory(
	photo: Photo,
	frames: Frame[],
	quality: number = 0.92,
	onProgress?: (current: number, total: number) => void
): Promise<string[]> {
	const directory = await open({
		directory: true,
		multiple: false,
		title: 'Select export directory'
	});

	if (!directory || typeof directory !== 'string') return [];

	const exportedPaths: string[] = [];

	for (let i = 0; i < frames.length; i++) {
		if (onProgress) onProgress(i + 1, frames.length);

		const blob = await cropFrameToBlob(photo, frames[i], quality);
		const buffer = await blob.arrayBuffer();
		const filename = generateFrameFilename(photo, frames[i], i);
		const fullPath = `${directory}/${filename}`;

		await writeFile(fullPath, new Uint8Array(buffer));
		exportedPaths.push(fullPath);
	}

	return exportedPaths;
}

export async function exportMetadata(
	photo: Photo,
	frames: Frame[],
	focalPoints: FocalPoint[],
	exportPath: string
): Promise<void> {
	const metadata = {
		photo: {
			title: photo.title,
			width: photo.width,
			height: photo.height,
			file_path: photo.file_path
		},
		frames: frames.map((f) => ({
			id: f.id,
			label: f.label,
			x: f.x,
			y: f.y,
			width: f.width,
			height: f.height
		})),
		focal_points: focalPoints.map((p) => ({
			id: p.id,
			frame_id: p.frame_id,
			label: p.label,
			x: p.x,
			y: p.y,
			required: p.required
		}))
	};

	const json = JSON.stringify(metadata, null, 2);
	const encoder = new TextEncoder();
	await writeFile(exportPath, encoder.encode(json));
}
