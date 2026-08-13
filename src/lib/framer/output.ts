import { objectPosition } from '../composition';
import { findDestination } from '../destinations';
import { type CropDoc, docToSidecar, type ExportFormat, type Frame } from './types';

// Browsers cannot encode AVIF, so a standalone export lands on WebP instead.
export type FileFormat = ExportFormat | 'webp';

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

export function basename(imageName: string): string {
	return imageName.replace(/\.[^/.]+$/, '').replace(/\s+/g, '-') || 'photo';
}

export function sidecarName(imageName: string): string {
	return `${basename(imageName)}.metadata.json`;
}

export function generateFrameFilename(
	photo: { title?: string },
	frame: Frame,
	index: number,
	format: FileFormat = 'avif',
	usedNames?: Set<string>
): string {
	const stem = basename(photo.title ?? '');
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

	return `${stem}.${name}.${ratio}.${format}`;
}

export function getDefaultExportDir(photoPath: string): string {
	const lastSlash = photoPath.lastIndexOf('/');
	return lastSlash > 0 ? photoPath.substring(0, lastSlash) : '/';
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

export function sidecarJson(doc: CropDoc, filePath?: string): string {
	return JSON.stringify({ ...docToSidecar(doc, filePath), fit: buildFitSection(doc.frames) }, null, 2);
}
