<script lang="ts">
import { onMount } from 'svelte';
import { goto } from '$app/navigation';
import { page } from '$app/stores';
import ExportDialog from '$lib/components/ExportDialog.svelte';
import ImageCanvas from '$lib/components/ImageCanvas.svelte';
import {
	addFocalPoint,
	addFrame,
	deleteFocalPoint,
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

type HistoryState = {
	frames: Frame[];
	focalPoints: FocalPoint[];
};

let photo = $state<Photo | null>(null);
let frames = $state<Frame[]>([]);
let focalPoints = $state<FocalPoint[]>([]);
let loading = $state(true);
let showExportDialog = $state(false);
let saveStatus = $state<'saved' | 'saving' | 'unsaved'>('saved');

const photoId = Number($page.params.id);

// State history for undo/redo
let history = $state<HistoryState[]>([]);
let historyIndex = $state(-1);
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

function captureSnapshot() {
	// Debounce snapshots (500ms delay)
	if (debounceTimer) clearTimeout(debounceTimer);
	debounceTimer = setTimeout(() => {
		// Remove any history after current index (if we've undone and then made changes)
		history = history.slice(0, historyIndex + 1);

		// Add new snapshot
		history.push({
			frames: structuredClone(frames),
			focalPoints: structuredClone(focalPoints)
		});

		// Limit history size
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
	try {
		// Save all frames
		for (const frame of frames) {
			await updateFrame(frame.id, {
				x: frame.x,
				y: frame.y,
				width: frame.width,
				height: frame.height,
				label: frame.label
			});
		}

		// Save all focal points
		for (const point of focalPoints) {
			await updateFocalPoint(point.id, {
				x: point.x,
				y: point.y,
				label: point.label,
				required: point.required
			});
		}

		saveStatus = 'saved';
	} catch (error) {
		console.error('Failed to save metadata:', error);
		saveStatus = 'unsaved';
	}
}

onMount(async () => {
	await loadPhoto();

	// Capture initial snapshot after load
	history.push({
		frames: structuredClone(frames),
		focalPoints: structuredClone(focalPoints)
	});
	historyIndex = 0;
});

// Watch for changes to frames/focal points and capture snapshots
$effect(() => {
	// Access frames and focalPoints to track them
	const _ = [frames, focalPoints];

	// Skip if we're still loading or if history is empty
	if (loading || history.length === 0) return;

	captureSnapshot();
});

async function loadPhoto() {
	loading = true;
	try {
		photo = await getPhoto(photoId);
		frames = await getFrames(photoId);
		focalPoints = await getFocalPoints(photoId);
	} catch (error) {
		console.error('Failed to load photo:', error);
	} finally {
		loading = false;
	}
}

async function handleAddFrame() {
	if (!photo) return;

	const id = await addFrame({
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

	const id = await addFocalPoint({
		photo_id: photoId,
		frame_id: undefined,
		x: 0.5,
		y: 0.5,
		label: `Point ${focalPoints.length + 1}`,
		required: true
	});

	await loadPhoto();
}

async function handleExport() {
	showExportDialog = true;
}

async function handleDeletePhoto() {
	if (!photo) return;
	if (confirm(`Delete "${photo.title}"? This will also delete all frames and focal points.`)) {
		await deletePhoto(photoId);
		goto('/');
	}
}
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="editor">
	<div class="toolbar">
		<a href="/">← Back</a>
		<div class="title">{photo?.title || 'Untitled'}</div>
		<div class="tools">
			<button onclick={handleAddFrame}>Add Frame</button>
			<button onclick={handleAddFocalPoint}>Add Focal Point</button>
			<button 
				onclick={saveMetadata} 
				class:save-btn-unsaved={saveStatus === 'unsaved'}
				disabled={saveStatus === 'saving' || saveStatus === 'saved'}
			>
				{saveStatus === 'saving' ? 'Saving...' : saveStatus === 'saved' ? 'Saved' : 'Save'}
			</button>
			<button onclick={handleExport}>Export</button>
			<button class="delete-btn" onclick={handleDeletePhoto}>Delete Photo</button>
		</div>
	</div>

	{#if loading}
		<div class="loading">Loading...</div>
	{:else if photo}
		<ImageCanvas {photo} bind:frames bind:focalPoints />
	{:else}
		<div class="error">Photo not found</div>
	{/if}
</div>

{#if showExportDialog && photo}
	<ExportDialog
		{photo}
		{frames}
		{focalPoints}
		onClose={() => showExportDialog = false}
	/>
{/if}

<style>
	.editor {
		height: 100vh;
		display: flex;
		flex-direction: column;
	}

	.toolbar {
		display: flex;
		align-items: center;
		gap: 2rem;
		padding: 1rem 2rem;
		border-bottom: 1px solid #e5e7eb;
		background: white;
	}

	.toolbar a {
		color: #2563eb;
		text-decoration: none;
	}

	.title {
		flex: 1;
		font-weight: 600;
	}

	.tools {
		display: flex;
		gap: 0.5rem;
	}

	button {
		padding: 0.5rem 1rem;
		background: #2563eb;
		color: white;
		border: none;
		border-radius: 0.375rem;
		cursor: pointer;
		font-size: 0.875rem;
	}

	button:hover:not(:disabled) {
		background: #1d4ed8;
	}

	button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	button.save-btn-unsaved {
		background: #f59e0b;
	}

	button.save-btn-unsaved:hover {
		background: #d97706;
	}

	button.delete-btn {
		background: #dc2626;
	}

	button.delete-btn:hover {
		background: #b91c1c;
	}

	.loading,
	.error {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		color: #6b7280;
	}
</style>
