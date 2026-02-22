<script lang="ts">
import { onMount } from 'svelte';
import { page } from '$app/stores';
import ExportDialog from '$lib/components/ExportDialog.svelte';
import ImageCanvas from '$lib/components/ImageCanvas.svelte';
import {
	addFocalPoint,
	addFrame,
	type FocalPoint,
	type Frame,
	getFocalPoints,
	getFrames,
	getPhoto,
	type Photo
} from '$lib/db';

let photo = $state<Photo | null>(null);
let frames = $state<Frame[]>([]);
let focalPoints = $state<FocalPoint[]>([]);
let loading = $state(true);
let showExportDialog = $state(false);

const photoId = Number($page.params.id);

onMount(async () => {
	await loadPhoto();
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
</script>

<div class="editor">
	<div class="toolbar">
		<a href="/">← Back</a>
		<div class="title">{photo?.title || 'Untitled'}</div>
		<div class="tools">
			<button onclick={handleAddFrame}>Add Frame</button>
			<button onclick={handleAddFocalPoint}>Add Focal Point</button>
			<button onclick={handleExport}>Export</button>
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

	button:hover {
		background: #1d4ed8;
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
