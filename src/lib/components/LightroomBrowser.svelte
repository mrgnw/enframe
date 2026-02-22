<script lang="ts">
import { onMount } from 'svelte';
import { importFile } from '$lib/db';
import { type LightroomPhoto, listLightroomPreviews } from '$lib/lightroom';
import { getImageSrc } from '$lib/tauri';

let photos = $state<LightroomPhoto[]>([]);
let loading = $state(false);
let error = $state<string | null>(null);
let importing = $state(false);

interface Props {
	onPhotosImported?: () => void;
}

let { onPhotosImported }: Props = $props();

onMount(async () => {
	await loadPreviews();
});

async function loadPreviews() {
	loading = true;
	error = null;
	try {
		photos = await listLightroomPreviews();
	} catch (err) {
		error = err instanceof Error ? err.message : 'Failed to load Lightroom previews';
	} finally {
		loading = false;
	}
}

async function handleImport(photo: LightroomPhoto) {
	if (!photo.previewPath) return;

	importing = true;
	try {
		await importFile(photo.previewPath, {
			width: photo.width,
			height: photo.height,
			title: photo.title
		});
		if (onPhotosImported) onPhotosImported();
	} catch (err) {
		console.error('Failed to import photo:', err);
	} finally {
		importing = false;
	}
}
</script>

<div class="lightroom-browser">
	<div class="header">
		<h2>Lightroom Library</h2>
		{#if !loading && !error}
			<p class="count">{photos.length} photos found</p>
		{/if}
	</div>

	{#if loading}
		<div class="status">Loading previews...</div>
	{:else if error}
		<div class="status error">
			{error}
			<button onclick={loadPreviews}>Retry</button>
		</div>
	{:else if photos.length === 0}
		<div class="status">No photos found in Lightroom library</div>
	{:else}
		<div class="grid">
			{#each photos as photo}
				<div class="photo-item">
					<div class="thumbnail">
						{#if photo.previewPath}
							<img src={getImageSrc(photo.previewPath)} alt="Photo {photo.assetId}" />
						{:else}
							<div class="no-preview">No preview</div>
						{/if}
					</div>
					<button
						class="import-btn"
						onclick={() => handleImport(photo)}
						disabled={importing || !photo.previewPath}
					>
						{importing ? 'Importing...' : 'Import'}
					</button>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.lightroom-browser {
		background: #f9fafb;
		border: 1px solid #e5e7eb;
		border-radius: 0.5rem;
		padding: 1.5rem;
		margin-bottom: 2rem;
	}

	.header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 1rem;
	}

	.header h2 {
		margin: 0;
		font-size: 1.25rem;
	}

	.count {
		color: #6b7280;
		font-size: 0.875rem;
		margin: 0;
	}

	.status {
		text-align: center;
		padding: 2rem;
		color: #6b7280;
	}

	.status.error {
		color: #dc2626;
	}

	.status button {
		margin-top: 0.5rem;
		padding: 0.5rem 1rem;
		background: #2563eb;
		color: white;
		border: none;
		border-radius: 0.375rem;
		cursor: pointer;
		font-size: 0.875rem;
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
		gap: 1rem;
		max-height: 400px;
		overflow-y: auto;
	}

	.photo-item {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.thumbnail {
		aspect-ratio: 3/2;
		overflow: hidden;
		background: #e5e7eb;
		border-radius: 0.375rem;
	}

	.thumbnail img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.no-preview {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		color: #9ca3af;
		font-size: 0.75rem;
	}

	.import-btn {
		padding: 0.375rem 0.75rem;
		background: #2563eb;
		color: white;
		border: none;
		border-radius: 0.375rem;
		cursor: pointer;
		font-size: 0.875rem;
	}

	.import-btn:hover {
		background: #1d4ed8;
	}

	.import-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
