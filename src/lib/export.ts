import { toast } from 'svelte-sonner';
import { pickDirectory as bridgePickDirectory, invoke, writeFile } from './bridge';
import type { Photo } from './db';
import { AVIF_SPEED, EXPORT_QUALITY } from './framer/canvas-export';
import { generateFrameFilename, sidecarJson } from './framer/output';
import type { CropDoc, ExportFormat, Frame } from './framer/types';
import { getLrAssetPaths } from './lightroom';

export type { ExportFormat };

export function pickDirectory(title = 'Select export directory'): Promise<string | null> {
	return bridgePickDirectory(title);
}

async function resolveExportSource(photo: Photo): Promise<string> {
	if (!photo.lr_asset_id) return photo.file_path;
	try {
		const [paths] = await getLrAssetPaths([photo.lr_asset_id]);
		if (paths?.proxy_path) return paths.proxy_path;
	} catch (error) {
		console.error('Failed to resolve Lightroom proxy path:', error);
		toast.warning('Lightroom proxy unavailable, exporting from original file');
	}
	return photo.file_path;
}

export async function exportFrames(
	photo: Photo,
	frames: Frame[],
	directory: string,
	format: ExportFormat = 'avif',
	quality = EXPORT_QUALITY,
	speed = AVIF_SPEED,
	onProgress?: (current: number, total: number) => void
): Promise<string[]> {
	const exportedPaths: string[] = [];
	const qualityU8 = Math.round(quality * 100);
	const sourcePath = await resolveExportSource(photo);
	const usedNames = new Set<string>();

	for (let i = 0; i < frames.length; i++) {
		if (onProgress) {
			onProgress(i, frames.length);
			await new Promise((r) => requestAnimationFrame(r));
		}

		const frame = frames[i];
		const filename = generateFrameFilename(photo, frame, i, format, usedNames);
		const fullPath = `${directory}/${filename}`;

		await invoke('crop_and_export', {
			sourcePath,
			outputPath: fullPath,
			x: frame.x,
			y: frame.y,
			width: frame.width,
			height: frame.height,
			quality: qualityU8,
			speed,
			format
		});

		exportedPaths.push(fullPath);
	}

	return exportedPaths;
}

export async function exportMetadata(photo: Photo, doc: CropDoc, exportPath: string): Promise<void> {
	const image = { name: photo.title ?? '', width: photo.width ?? 0, height: photo.height ?? 0 };
	const json = sidecarJson({ ...doc, image }, photo.file_path);
	await writeFile(exportPath, new TextEncoder().encode(json));
}
