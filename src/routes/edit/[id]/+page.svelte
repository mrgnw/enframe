<script lang="ts">
import { onMount } from 'svelte';
import { toast } from 'svelte-sonner';
import { page } from '$app/stores';
import { getImageSrc } from '$lib/bridge';
import {
	addFocalPoint,
	addFrame,
	appendHistory,
	deleteFocalPoint,
	deleteFrame,
	deleteHistorySeqs,
	getFocalPoints,
	getFrames,
	getHistorySeqs,
	getHistorySnapshot,
	getPhoto,
	type Photo,
	restoreSnapshot,
	setHistoryPos,
	updateFocalPoint,
	updateFrame
} from '$lib/db';
import { exportFrames, exportMetadata, getDefaultExportDir, pickDirectory } from '$lib/export';
import Framer from '$lib/framer/Framer.svelte';
import type { FramerStore } from '$lib/framer/session.svelte';
import { type CropDoc, CROP_DOC_VERSION, type ExportFormat, type Frame } from '$lib/framer/types';
import { parseSnapshot } from '$lib/history';

type Report = (done: number, total: number) => void;

function yieldFrame(): Promise<void> {
	return new Promise((r) => requestAnimationFrame(() => r()));
}

const photoId = Number($page.params.id);

let photo = $state<Photo | null>(null);
let doc = $state<CropDoc | null>(null);
let historySeqs = $state<number[]>([]);
let historyPos = $state(0);
let loading = $state(true);

let exportDir = $state('');
let exportFormat = $state<ExportFormat>('avif');

const store: FramerStore = {
	addFrame: (frame) => addFrame({ photo_id: photoId, ...frame }),
	updateFrame,
	deleteFrame,
	addPoint: (point) => addFocalPoint({ photo_id: photoId, ...point }),
	updatePoint: updateFocalPoint,
	deletePoint: deleteFocalPoint,
	async saveHistory(seq, drop, snapshot) {
		await deleteHistorySeqs(photoId, drop);
		await appendHistory(photoId, seq, snapshot);
		await setHistoryPos(photoId, seq);
	},
	async loadHistory(seq) {
		const stored = await getHistorySnapshot(photoId, seq);
		if (stored === null) return null;
		const frameIds = await restoreSnapshot(photoId, parseSnapshot(stored));
		await setHistoryPos(photoId, seq);
		return { frames: await getFrames(photoId), focalPoints: await getFocalPoints(photoId), frameIds };
	}
};

onMount(async () => {
	try {
		photo = await getPhoto(photoId);
		if (photo) {
			exportDir = getDefaultExportDir(photo.file_path);
			doc = {
				version: CROP_DOC_VERSION,
				image: { name: photo.title ?? '', width: photo.width ?? 0, height: photo.height ?? 0 },
				frames: await getFrames(photoId),
				focalPoints: await getFocalPoints(photoId)
			};
			historySeqs = await getHistorySeqs(photoId);
			historyPos = photo.history_pos ?? 0;
		}
	} catch (error) {
		console.error('Failed to load photo:', error);
		toast.error('Failed to load photo');
	} finally {
		loading = false;
	}
});

async function save(edited: CropDoc, report: Report) {
	const total = edited.frames.length + edited.focalPoints.length;
	let done = 0;
	report(done, total);
	await yieldFrame();
	for (const frame of edited.frames) {
		await updateFrame(frame.id, {
			x: frame.x,
			y: frame.y,
			width: frame.width,
			height: frame.height,
			label: frame.label
		});
		report(++done, total);
		await yieldFrame();
	}
	for (const point of edited.focalPoints) {
		await updateFocalPoint(point.id, {
			x: point.x,
			y: point.y,
			label: point.label,
			required: point.required
		});
		report(++done, total);
		await yieldFrame();
	}
}

async function runExport(frames: Frame[], edited: CropDoc, report: Report) {
	if (!photo) return;
	const total = frames.length + 1;
	report(0, total);
	await yieldFrame();
	try {
		await exportFrames(photo, frames, exportDir, exportFormat, 0.92, (current: number) => report(current, total));
		report(frames.length, total);
		await yieldFrame();
		const basename = photo.title?.replace(/\.[^/.]+$/, '').replace(/\s+/g, '-') || 'photo';
		await exportMetadata(photo, { ...edited, frames }, `${exportDir}/${basename}.metadata.json`);
		report(total, total);
		toast.success(`Exported ${frames.length} frame${frames.length === 1 ? '' : 's'}`);
	} catch (error) {
		console.error('Export failed:', error);
		toast.error('Export failed');
	}
}

async function setOutputDir() {
	const dir = await pickDirectory();
	if (dir) exportDir = dir;
}

function setFormat(format: ExportFormat) {
	exportFormat = format;
	toast.success(`Export format: ${format.toUpperCase()}`);
}
</script>

<div class="editor" data-tauri-drag-region>
	<div class="stoplight-area">
		<div class="stoplight-shadow"></div>
		<a href="/" class="back-btn" aria-label="Back">
			<svg class="back-chevron" width="14" height="14" viewBox="0 0 20 20" fill="none">
				<path d="M12 15L7 10L12 5" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
			<span class="back-label">Back</span>
		</a>
	</div>

	{#if loading}
		<div class="loading">Loading...</div>
	{:else if photo && doc}
		<Framer
			{doc}
			{store}
			{historySeqs}
			{historyPos}
			imageSrc={getImageSrc(photo.file_path)}
			onsave={save}
			onexport={runExport}
			onsetoutputdir={setOutputDir}
			onsetformat={setFormat}
		/>
	{:else}
		<div class="loading">Photo not found</div>
	{/if}
</div>

<style>
	.editor {
		width: 100vw;
		height: 100vh;
		overflow: hidden;
		background: #1a1a1a;
	}

	.stoplight-area {
		position: fixed;
		top: 0;
		left: 0;
		z-index: 50;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
	}

	.stoplight-shadow {
		width: 86px;
		height: 40px;
		background: radial-gradient(ellipse at 40px 16px, rgba(0, 0, 0, 0.4) 0%, transparent 70%);
		pointer-events: none;
	}

	.back-btn {
		display: flex;
		align-items: center;
		gap: 0;
		width: 26px;
		height: 26px;
		margin-left: 10px;
		padding: 0 6px;
		color: rgba(255, 255, 255, 0.85);
		text-decoration: none;
		font-size: 12px;
		font-weight: 500;
		background: rgba(0, 0, 0, 0.45);
		backdrop-filter: blur(8px);
		border-radius: 999px;
		overflow: hidden;
		transition: width 0.2s ease, color 0.15s, background 0.15s;
	}

	.back-btn:hover {
		width: 72px;
		color: white;
		background: rgba(0, 0, 0, 0.6);
	}

	.back-chevron {
		flex-shrink: 0;
	}

	.back-label {
		opacity: 0;
		margin-left: 3px;
		white-space: nowrap;
		transition: opacity 0.15s 0.05s;
	}

	.back-btn:hover .back-label {
		opacity: 1;
	}

	.loading {
		width: 100%;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		color: #6b7280;
	}
</style>
