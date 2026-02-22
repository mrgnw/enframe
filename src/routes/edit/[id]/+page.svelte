<script lang="ts">
import { onMount } from 'svelte';
import { toast } from 'svelte-sonner';
import { goto } from '$app/navigation';
import { page } from '$app/stores';
import CommandBar from '$lib/components/CommandBar.svelte';
import ImageCanvas from '$lib/components/ImageCanvas.svelte';
import {
	addFocalPoint,
	addFrame,
	deleteFrame,
	deletePhoto,
	type FocalPoint,
	type Frame,
	getFocalPoints,
	getFrames,
	getPhoto,
	type Photo,
	updateFocalPoint,
	updateFrame
} from '$lib/db';
import { exportFrames, exportMetadata, getDefaultExportDir, pickDirectory } from '$lib/export';

type HistoryState = {
	frames: Frame[];
	focalPoints: FocalPoint[];
};

let photo = $state<Photo | null>(null);
let frames = $state<Frame[]>([]);
let focalPoints = $state<FocalPoint[]>([]);
let loading = $state(true);

let saveStatus = $state<'saved' | 'saving' | 'unsaved'>('saved');
let saveProgress = $state(0);
let saveTotal = $state(0);

const photoId = Number($page.params.id);

let history = $state<HistoryState[]>([]);
let historyIndex = $state(-1);
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

let exportDir = $state('');

function captureSnapshot() {
	if (debounceTimer) clearTimeout(debounceTimer);
	debounceTimer = setTimeout(() => {
		history = history.slice(0, historyIndex + 1);
		history.push({
			frames: structuredClone(frames),
			focalPoints: structuredClone(focalPoints)
		});
		if (history.length > 50) {
			history = history.slice(-50);
		}
		historyIndex = history.length - 1;
		saveStatus = 'unsaved';
	}, 500);
}

function undo() {
	if (historyIndex <= 0) return;
	historyIndex--;
	const state = history[historyIndex];
	frames = structuredClone(state.frames);
	focalPoints = structuredClone(state.focalPoints);
}

function redo() {
	if (historyIndex >= history.length - 1) return;
	historyIndex++;
	const state = history[historyIndex];
	frames = structuredClone(state.frames);
	focalPoints = structuredClone(state.focalPoints);
}

function handleKeyDown(event: KeyboardEvent) {
	if ((event.metaKey || event.ctrlKey) && event.key === 'z') {
		event.preventDefault();
		if (event.shiftKey) {
			redo();
		} else {
			undo();
		}
	} else if ((event.metaKey || event.ctrlKey) && event.key === 's') {
		event.preventDefault();
		saveMetadata();
	}
}

async function saveMetadata() {
	saveStatus = 'saving';
	saveProgress = 0;
	saveTotal = frames.length + focalPoints.length;
	try {
		for (const frame of frames) {
			await updateFrame(frame.id, {
				x: frame.x,
				y: frame.y,
				width: frame.width,
				height: frame.height,
				label: frame.label
			});
			saveProgress++;
		}
		for (const point of focalPoints) {
			await updateFocalPoint(point.id, {
				x: point.x,
				y: point.y,
				label: point.label,
				required: point.required
			});
			saveProgress++;
		}
		saveStatus = 'saved';
		toast.success('Saved');
	} catch (error) {
		console.error('Failed to save metadata:', error);
		saveStatus = 'unsaved';
		toast.error('Save failed');
	}
}

onMount(async () => {
	await loadPhoto();
	history.push({
		frames: structuredClone(frames),
		focalPoints: structuredClone(focalPoints)
	});
	historyIndex = 0;
});

function handleChange() {
	if (loading || history.length === 0) return;
	captureSnapshot();
}

async function loadPhoto() {
	loading = true;
	try {
		photo = await getPhoto(photoId);
		frames = await getFrames(photoId);
		focalPoints = await getFocalPoints(photoId);
		if (photo) {
			exportDir = getDefaultExportDir(photo.file_path);
		}
	} catch (error) {
		console.error('Failed to load photo:', error);
	} finally {
		loading = false;
	}
}

async function handleAddFrame() {
	if (!photo) return;
	await addFrame({
		photo_id: photoId,
		label: `Frame ${frames.length + 1}`,
		x: 0.2,
		y: 0.2,
		width: 0.6,
		height: 0.6,
		is_default: false
	});
	await loadPhoto();
}

async function handleAddFocalPoint() {
	if (!photo) return;
	await addFocalPoint({
		photo_id: photoId,
		frame_id: undefined,
		x: 0.5,
		y: 0.5,
		label: `Point ${focalPoints.length + 1}`,
		required: true
	});
	await loadPhoto();
}

async function handleDeleteFrame(frameId: number) {
	await deleteFrame(frameId);
	await loadPhoto();
	saveStatus = 'unsaved';
}

async function handleExport() {
	if (!photo || frames.length === 0) return;
	try {
		await exportFrames(photo, frames, exportDir, 0.92);
		const basename = photo.title?.replace(/\.[^/.]+$/, '').replace(/\s+/g, '-') || 'photo';
		await exportMetadata(photo, frames, focalPoints, `${exportDir}/${basename}.metadata.json`);
	} catch (error) {
		console.error('Export failed:', error);
	}
}

async function handleSetOutputDir() {
	const dir = await pickDirectory();
	if (dir) {
		exportDir = dir;
	}
}
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="editor">
	{#if saveStatus === 'saving' && saveTotal > 0}
		<div class="save-progress">
			<div class="save-progress-bar" style="width:{(saveProgress / saveTotal) * 100}%"></div>
		</div>
	{/if}

	<a href="/" class="back-btn" aria-label="Back">
		<svg width="20" height="20" viewBox="0 0 20 20" fill="none">
			<path d="M12.5 15L7.5 10L12.5 5" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
		</svg>
	</a>

	{#if loading}
		<div class="loading">Loading...</div>
	{:else if photo}
		<ImageCanvas {photo} bind:frames bind:focalPoints onchange={handleChange} ondeleteframe={handleDeleteFrame} />
	{:else}
		<div class="loading">Photo not found</div>
	{/if}

	<CommandBar
		onaddframe={handleAddFrame}
		onaddpoint={handleAddFocalPoint}
		onsave={saveMetadata}
		onexport={handleExport}
		onsetoutputdir={handleSetOutputDir}
	/>
</div>

<style>
	.editor {
		width: 100vw;
		height: 100vh;
		overflow: hidden;
		background: #1a1a1a;
	}

	.back-btn {
		position: fixed;
		top: 1rem;
		left: 1rem;
		z-index: 50;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 2.5rem;
		height: 2.5rem;
		border-radius: 0.5rem;
		background: rgba(0, 0, 0, 0.5);
		color: rgba(255, 255, 255, 0.8);
		text-decoration: none;
		backdrop-filter: blur(8px);
		transition: background 0.15s, color 0.15s;
	}

	.back-btn:hover {
		background: rgba(0, 0, 0, 0.7);
		color: white;
	}

	.loading {
		width: 100%;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		color: #6b7280;
	}

	.save-progress {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		height: 3px;
		z-index: 100;
		background: rgba(255, 255, 255, 0.1);
	}

	.save-progress-bar {
		height: 100%;
		background: #3b82f6;
		transition: width 0.15s ease;
	}
</style>
