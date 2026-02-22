<script lang="ts">
import { open } from '@tauri-apps/plugin-dialog';
import { onMount } from 'svelte';
import { type Frame, importFile, listAllFrames, listPhotos, type Photo } from '$lib/db';
import { getFilePathFromDrop, getImageSrc } from '$lib/tauri';
import LightroomBrowser from './LightroomBrowser.svelte';

async function loadImageDimensions(src: string): Promise<{ width: number; height: number }> {
	return new Promise((resolve, reject) => {
		const img = new Image();
		img.onload = () => resolve({ width: img.naturalWidth, height: img.naturalHeight });
		img.onerror = reject;
		img.src = src;
	});
}

let photos = $state<Photo[]>([]);
let framesByPhoto = $state<Record<number, Frame[]>>({});
let loading = $state(true);
let importing = $state(false);
let showLightroomBrowser = $state(false);

onMount(async () => {
	await loadPhotos();
});

async function loadPhotos() {
	loading = true;
	try {
		photos = await listPhotos();
		const allFrames = await listAllFrames();
		const grouped: Record<number, Frame[]> = {};
		for (const f of allFrames) {
			if (!grouped[f.photo_id]) grouped[f.photo_id] = [];
			grouped[f.photo_id].push(f);
		}
		framesByPhoto = grouped;
	} catch (error) {
		console.error('Failed to load photos:', error);
	} finally {
		loading = false;
	}
}

async function handleFileDrop(event: DragEvent) {
	event.preventDefault();
	const files = event.dataTransfer?.files;
	if (!files || files.length === 0) return;

	importing = true;
	try {
		for (const file of Array.from(files)) {
			const path = await getFilePathFromDrop(file);
			if (path) {
				const src = getImageSrc(path);
				const dims = await loadImageDimensions(src);
				await importFile(path, dims);
			} else {
				console.warn('No file path available for dropped file:', file.name);
			}
		}
		await loadPhotos();
	} catch (error) {
		console.error('Failed to import files:', error);
	} finally {
		importing = false;
	}
}

function handleDragOver(event: DragEvent) {
	event.preventDefault();
}

async function handleImportClick() {
	const selected = await open({
		multiple: true,
		filters: [
			{
				name: 'Images',
				extensions: ['jpg', 'jpeg', 'png', 'jxl', 'tiff', 'tif', 'webp', 'heic']
			}
		]
	});

	if (!selected) return;

	importing = true;
	try {
		const files = Array.isArray(selected) ? selected : [selected];
		for (const path of files) {
			const src = getImageSrc(path);
			const dims = await loadImageDimensions(src);
			await importFile(path, dims);
		}
		await loadPhotos();
	} catch (error) {
		console.error('Failed to import files:', error);
	} finally {
		importing = false;
	}
}

function handleLightroomClick() {
	showLightroomBrowser = !showLightroomBrowser;
}
</script>

<div class="photo-browser" role="application" ondrop={handleFileDrop} ondragover={handleDragOver}>
	<div class="header" data-tauri-drag-region>
		<h1>Enframe</h1>
		<div class="actions">
			<button onclick={handleImportClick} disabled={importing}>
				{importing ? 'Importing...' : 'Import from Files'}
			</button>
			<button onclick={handleLightroomClick}>
				{showLightroomBrowser ? 'Hide Lightroom' : 'Browse Lightroom'}
			</button>
		</div>
	</div>

	{#if showLightroomBrowser}
		<LightroomBrowser onPhotosImported={loadPhotos} />
	{/if}

	{#if loading}
		<div class="loading">Loading photos...</div>
	{:else if importing}
		<div class="loading">Importing files...</div>
	{:else if photos.length === 0}
		<div class="empty">
			<p>No photos yet</p>
			<p class="hint">Drop images here or import from Lightroom</p>
		</div>
	{:else}
		<div class="grid">
			{#each photos as photo}
				<a href="/edit/{photo.id}" class="photo-card">
					<div class="thumbnail">
						<img src={getImageSrc(photo.file_path)} alt={photo.title || 'Photo'} />
						{#if framesByPhoto[photo.id]?.length}
							{@const pframes = framesByPhoto[photo.id]}
							<div class="frame-overlays">
								<svg class="frame-dimming" viewBox="0 0 1 1" preserveAspectRatio="none">
									<defs>
										<mask id="mask-{photo.id}">
											<rect x="0" y="0" width="1" height="1" fill="white" />
											{#each pframes as frame}
												<rect x={frame.x} y={frame.y} width={frame.width} height={frame.height} fill="black" />
											{/each}
										</mask>
									</defs>
									<rect x="0" y="0" width="1" height="1" fill="rgba(0,0,0,0.5)" mask="url(#mask-{photo.id})" />
								</svg>
								{#each pframes as frame, i}
									<div
										class="frame-preview"
										style="left:{frame.x * 100}%;top:{frame.y * 100}%;width:{frame.width * 100}%;height:{frame.height * 100}%;"
									>
										<span class="frame-preview-label">{i + 1}</span>
									</div>
								{/each}
							</div>
						{/if}
					</div>
					{#if photo.title}
						<div class="title">{photo.title}</div>
					{/if}
				</a>
			{/each}
		</div>
	{/if}
</div>

<style>
	.photo-browser {
		padding: 2rem;
		padding-top: 3.5rem;
		min-height: 100vh;
	}

	.header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 2rem;
	}

	.actions {
		display: flex;
		gap: 1rem;
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

	button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.loading,
	.empty {
		text-align: center;
		padding: 4rem 2rem;
		color: #6b7280;
	}

	.hint {
		font-size: 0.875rem;
		margin-top: 0.5rem;
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
		align-items: start;
		gap: 1.5rem;
	}

	.photo-card {
		display: block;
		position: relative;
		border-radius: 0.5rem;
		overflow: hidden;
		background: #1a1a1a;
		transition: transform 0.2s;
		text-decoration: none;
		color: inherit;
	}

	.photo-card:hover {
		transform: translateY(-4px);
	}

	.thumbnail {
		position: relative;
		aspect-ratio: 3/2;
		overflow: hidden;
		background: #e5e7eb;
	}

	.thumbnail img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.frame-overlays {
		position: absolute;
		inset: 0;
		opacity: 0;
		transition: opacity 0.2s;
		pointer-events: none;
	}

	.photo-card:hover .frame-overlays {
		opacity: 1;
	}

	.frame-dimming {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
	}

	.frame-preview {
		position: absolute;
		border: 1.5px solid rgba(59, 130, 246, 0.8);
		box-sizing: border-box;
	}

	.frame-preview-label {
		position: absolute;
		top: 2px;
		left: 2px;
		background: rgba(59, 130, 246, 0.8);
		color: white;
		font-size: 9px;
		font-weight: 600;
		padding: 0 3px;
		border-radius: 2px;
		line-height: 14px;
	}

	.title {
		display: none;
		padding: 0.5rem 0.6rem;
		font-size: 0.75rem;
		font-weight: 500;
		color: #ccc;
	}

	.photo-card:hover .title {
		display: block;
	}
</style>
