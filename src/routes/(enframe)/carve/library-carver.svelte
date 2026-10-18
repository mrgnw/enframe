<script lang="ts">
// The carve screen for a library photo. Saving renders the version at full size, writes it next
// to the source and adds it to the library as a photo of its own, with the focal points carried
// through the carve.
import { onMount } from 'svelte';
import { toast } from 'svelte-sonner';
import { goto } from '$app/navigation';
import { page } from '$app/stores';
import { getImageSrc, writeFile } from '$lib/bridge';
import Carver, { type CarveResult } from '$lib/carve/Carver.svelte';
import type { CarvePoint, CarveRecord } from '$lib/carve/types';
import {
	addFocalPoint,
	addPhoto,
	type FocalPoint,
	getFocalPoints,
	getPhoto,
	getPhotoByPath,
	type Photo
} from '$lib/db';
import { pickDirectory } from '$lib/export';
import { avifQuality, canvasBlob, EXPORT_QUALITY, encodingFor } from '$lib/framer/canvas-export';
import { basename, getDefaultExportDir } from '$lib/framer/output';
import type { ExportFormat } from '$lib/framer/types';

const photoId = Number($page.params.id);
const FOCAL_DISC = 0.1;
const FORMATS: ExportFormat[] = ['avif', 'jpg', 'png'];

let photo = $state<Photo | null>(null);
let focalPoints = $state<FocalPoint[]>([]);
let loading = $state(true);
let format = $state<ExportFormat>('avif');

onMount(async () => {
	try {
		photo = await getPhoto(photoId);
		if (photo) focalPoints = await getFocalPoints(photoId);
	} catch (error) {
		console.error('Failed to load photo:', error);
		toast.error('Failed to load photo');
	} finally {
		loading = false;
	}
});

const initialPoints = $derived<CarvePoint[]>(
	focalPoints.map((p) => ({ kind: 'protect', x: p.x, y: p.y, size: FOCAL_DISC, strength: 10 }))
);

async function toBlob(result: CarveResult): Promise<Blob> {
	const encoding = encodingFor(format);
	if (encoding.extension === 'avif') {
		const { encodeAvif } = await import('$lib/framer/avif');
		return new Blob([await encodeAvif(result.pixels, avifQuality(EXPORT_QUALITY))], { type: encoding.mime });
	}
	const canvas = document.createElement('canvas');
	canvas.width = result.width;
	canvas.height = result.height;
	canvas.getContext('2d')?.putImageData(result.pixels, 0, 0);
	return canvasBlob(canvas, encoding.mime, EXPORT_QUALITY);
}

const fileName = (source: Photo, result: CarveResult, suffix: string) =>
	`${basename(source.title ?? '')}.carved${suffix}.${result.ratio}.${encodingFor(format).extension}`;

// ponytail: only the library is asked whether the name is taken; a stray file with the same name
// that was never imported gets overwritten.
async function freePath(source: Photo, result: CarveResult): Promise<string> {
	const dir = getDefaultExportDir(source.file_path);
	let path = `${dir}/${fileName(source, result, '')}`;
	for (let n = 2; await getPhotoByPath(path); n++) path = `${dir}/${fileName(source, result, `-${n}`)}`;
	return path;
}

async function save(result: CarveResult) {
	if (!photo) return;
	try {
		const path = await freePath(photo, result);
		const blob = await toBlob(result);
		await writeFile(path, new Uint8Array(await blob.arrayBuffer()));
		const record: CarveRecord = { settings: result.settings, points: result.points };
		const id = await addPhoto({
			file_path: path,
			width: result.width,
			height: result.height,
			title: `${photo.title ?? basename(path)} carved ${result.ratio}`,
			description: undefined,
			lr_asset_id: undefined,
			source_id: photo.id,
			carve: JSON.stringify(record)
		});
		for (const point of focalPoints) {
			const at = result.map(point);
			if (at) await addFocalPoint({ photo_id: id, x: at.x, y: at.y, label: point.label, required: point.required });
		}
		toast.success('Saved to the library');
		await goto(`/edit/${id}`);
	} catch (error) {
		console.error('Save failed:', error);
		toast.error('Save failed');
	}
}

async function exportFile(result: CarveResult) {
	if (!photo) return;
	const dir = await pickDirectory();
	if (!dir) return;
	try {
		const blob = await toBlob(result);
		const path = `${dir}/${fileName(photo, result, '')}`;
		await writeFile(path, new Uint8Array(await blob.arrayBuffer()));
		toast.success(`Exported ${path.split('/').pop()}`);
	} catch (error) {
		console.error('Export failed:', error);
		toast.error('Export failed');
	}
}
</script>

<div class="screen">
	<a href="/edit/{photoId}" class="back" aria-label="Back to the framer" title="Back to the framer">‹</a>
	{#if loading}
		<div class="loading">Loading...</div>
	{:else if photo}
		<Carver imageSrc={getImageSrc(photo.file_path)} title={photo.title ?? ''} {initialPoints} onsave={save} onexport={exportFile} />
		<div class="format seg">
			{#each FORMATS as f (f)}
				<button class:on={format === f} onclick={() => (format = f)}>{f.toUpperCase()}</button>
			{/each}
		</div>
	{:else}
		<div class="loading">Photo not found</div>
	{/if}
</div>

<style>
	.screen {
		position: relative;
		min-height: 100vh;
		background: var(--bg);
	}
	.back {
		position: fixed;
		top: 18px;
		left: 90px;
		z-index: 61;
		width: 30px;
		height: 30px;
		display: grid;
		place-items: center;
		border-radius: 999px;
		background: var(--surface);
		color: var(--fg);
		text-decoration: none;
		font-size: 18px;
	}
	.format {
		position: fixed;
		right: 16px;
		bottom: 14px;
		z-index: 6;
	}
	.loading {
		height: 100vh;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--subtle);
	}
</style>
