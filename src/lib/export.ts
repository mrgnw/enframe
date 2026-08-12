import { toast } from 'svelte-sonner';
import { pickDirectory as bridgePickDirectory, invoke, writeFile } from './bridge';
import { objectPosition } from './composition';
import type { FocalPoint, Frame, Photo } from './db';
import { findDestination } from './destinations';
import { getLrAssetPaths } from './lightroom';

export type ExportFormat = 'avif' | 'jpg' | 'png';

function slugify(value: string): string {
	return value
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-+|-+$/g, '');
}

function frameNameSegment(frame: Frame, index: number): string {
	if (frame.destination) return frame.destination;
	const labelSlug = frame.label ? slugify(frame.label) : '';
	if (labelSlug) return labelSlug;
	const orientation = frame.width > frame.height ? 'l' : frame.width < frame.height ? 'p' : 's';
	return `${orientation}.${index + 1}`;
}

export function generateFrameFilename(
	photo: Photo,
	frame: Frame,
	index: number,
	format: ExportFormat = 'avif',
	usedNames?: Set<string>
): string {
	const basename = photo.title?.replace(/\.[^/.]+$/, '').replace(/\s+/g, '-') || 'photo';
	const ratio = (frame.width / frame.height).toFixed(2);

	let name = frameNameSegment(frame, index);
	if (usedNames) {
		const base = name;
		let suffix = 2;
		while (usedNames.has(name)) {
			name = `${base}-${suffix}`;
			suffix += 1;
		}
		usedNames.add(name);
	}

	return `${basename}.${name}.${ratio}.${format}`;
}

export function getDefaultExportDir(photoPath: string): string {
	const lastSlash = photoPath.lastIndexOf('/');
	return lastSlash > 0 ? photoPath.substring(0, lastSlash) : '/';
}

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
	quality: number = 0.92,
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
			format
		});

		exportedPaths.push(fullPath);
	}

	return exportedPaths;
}

// Per-destination crops for render-time fitting: object_position approximates the
// crop for consumers using CSS `object-fit: cover` on a container of the same ratio.
function buildFitSection(frames: Frame[]) {
	const fit: {
		slug: string;
		ratio: number;
		crop: { x: number; y: number; width: number; height: number };
		object_position: string;
	}[] = [];

	for (const frame of frames) {
		const destination = findDestination(frame.destination);
		if (!destination) continue;
		const crop = { x: frame.x, y: frame.y, width: frame.width, height: frame.height };
		fit.push({
			slug: destination.slug,
			ratio: destination.ratio.w / destination.ratio.h,
			crop,
			object_position: objectPosition(crop)
		});
	}

	return fit;
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
			destination: f.destination ?? null,
			is_auto: f.is_auto,
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
		})),
		fit: buildFitSection(frames)
	};

	const json = JSON.stringify(metadata, null, 2);
	const encoder = new TextEncoder();
	await writeFile(exportPath, encoder.encode(json));
}
