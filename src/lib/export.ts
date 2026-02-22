import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { writeFile } from '@tauri-apps/plugin-fs';
import type { FocalPoint, Frame, Photo } from './db';

function generateFrameFilename(photo: Photo, frame: Frame, index: number): string {
	const basename = photo.title?.replace(/\.[^/.]+$/, '').replace(/\s+/g, '-') || 'photo';
	const ratio = (frame.width / frame.height).toFixed(2);
	const orientation = frame.width > frame.height ? 'l' : frame.width < frame.height ? 'p' : 's';
	return `${basename}.${orientation}.${index + 1}.${ratio}.jpg`;
}

export function getDefaultExportDir(photoPath: string): string {
	const lastSlash = photoPath.lastIndexOf('/');
	return lastSlash > 0 ? photoPath.substring(0, lastSlash) : '/';
}

export async function pickDirectory(title = 'Select export directory'): Promise<string | null> {
	const result = await open({
		directory: true,
		multiple: false,
		title
	});
	if (!result || typeof result !== 'string') return null;
	return result;
}

export async function exportFrames(
	photo: Photo,
	frames: Frame[],
	directory: string,
	quality: number = 0.92,
	onProgress?: (current: number, total: number) => void
): Promise<string[]> {
	const exportedPaths: string[] = [];
	const qualityU8 = Math.round(quality * 100);

	for (let i = 0; i < frames.length; i++) {
		if (onProgress) onProgress(i + 1, frames.length);

		const frame = frames[i];
		const filename = generateFrameFilename(photo, frame, i);
		const fullPath = `${directory}/${filename}`;

		await invoke('crop_and_export', {
			sourcePath: photo.file_path,
			outputPath: fullPath,
			x: frame.x,
			y: frame.y,
			width: frame.width,
			height: frame.height,
			quality: qualityU8
		});

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
