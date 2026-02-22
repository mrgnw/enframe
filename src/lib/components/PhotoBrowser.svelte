<script lang="ts">
import { Aperture, ChevronDown, ChevronUp, FolderPlus } from '@lucide/svelte';
import { open } from '@tauri-apps/plugin-dialog';
import { onMount } from 'svelte';
import { type Frame, importFile, listAllFrames, listPhotos, type Photo } from '$lib/db';
import { getFilePathFromDrop, getImageSrc } from '$lib/tauri';
import LightroomBrowser from './LightroomBrowser.svelte';
import WatchDir from './WatchDir.svelte';

function loadImageDimensions(src: string): Promise<{ width: number; height: number }> {
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
let showLightroom = $state(false);
let openDragOver = $state(false);

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

async function importFiles(paths: string[]) {
	importing = true;
	try {
		for (const path of paths) {
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

async function handleOpenClick() {
	const selected = await open({
		multiple: true,
		filters: [
			{
				name: 'Images',
				extensions: ['jpg', 'jpeg', 'png', 'jxl', 'tiff', 'tif', 'webp', 'heic', 'dng', 'arw']
			}
		]
	});
	if (!selected) return;
	const files = Array.isArray(selected) ? selected : [selected];
	await importFiles(files);
}

async function handleOpenDrop(e: DragEvent) {
	e.preventDefault();
	e.stopPropagation();
	openDragOver = false;
	const files = e.dataTransfer?.files;
	if (!files || files.length === 0) return;

	const paths: string[] = [];
	for (const file of Array.from(files)) {
		const path = await getFilePathFromDrop(file);
		if (path) paths.push(path);
	}
	if (paths.length > 0) await importFiles(paths);
}

function handleOpenDragOver(e: DragEvent) {
	e.preventDefault();
	e.stopPropagation();
	openDragOver = true;
}

function handleOpenDragLeave(e: DragEvent) {
	e.preventDefault();
	openDragOver = false;
}

let bodyDragOver = $state(false);

async function handleBodyDrop(e: DragEvent) {
	e.preventDefault();
	bodyDragOver = false;
	const files = e.dataTransfer?.files;
	if (!files || files.length === 0) return;

	const paths: string[] = [];
	for (const file of Array.from(files)) {
		const path = await getFilePathFromDrop(file);
		if (path) paths.push(path);
	}
	if (paths.length > 0) await importFiles(paths);
}

function handleBodyDragOver(e: DragEvent) {
	e.preventDefault();
	bodyDragOver = true;
}

function handleBodyDragLeave(e: DragEvent) {
	if (e.currentTarget === e.target) {
		bodyDragOver = false;
	}
}
</script>

<div
	class="photo-browser"
	class:body-drag-over={bodyDragOver}
	role="application"
	ondrop={handleBodyDrop}
	ondragover={handleBodyDragOver}
	ondragleave={handleBodyDragLeave}
>
	<div class="header" data-tauri-drag-region>
		<h1>Enframe</h1>
		<div class="actions">
			<WatchDir onPhotosImported={loadPhotos} />

			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<button
				class="action-pill open-pill"
				class:drag-over={openDragOver}
				onclick={handleOpenClick}
				disabled={importing}
				ondrop={handleOpenDrop}
				ondragover={handleOpenDragOver}
				ondragleave={handleOpenDragLeave}
			>
				<FolderPlus size={14} />
				{importing ? 'Importing...' : 'Open'}
			</button>

			<button
				class="action-pill lr-pill"
				class:active={showLightroom}
				onclick={() => (showLightroom = !showLightroom)}
			>
				<Aperture size={14} />
				Lightroom
				{#if showLightroom}
					<ChevronUp size={12} />
				{:else}
					<ChevronDown size={12} />
				{/if}
			</button>
		</div>
	</div>

	{#if showLightroom}
		<LightroomBrowser onPhotosImported={loadPhotos} />
	{/if}

	{#if loading}
		<div class="loading">Loading photos...</div>
	{:else if importing}
		<div class="loading">Importing files...</div>
	{:else if photos.length === 0}
		<div class="empty">
			<p>No photos yet</p>
			<p class="hint">Drop images onto Open, or browse Lightroom</p>
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
		transition: outline-color 0.15s;
		outline: 2px dashed transparent;
		outline-offset: -2px;
	}

	.photo-browser.body-drag-over {
		outline-color: #2563eb;
	}

	.header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 1.5rem;
	}

	.header h1 {
		margin: 0;
	}

	.actions {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.action-pill {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		padding: 0.35rem 0.65rem;
		background: #1a1a1a;
		border: 1px solid #333;
		border-radius: 0.375rem;
		color: #ccc;
		font-size: 0.75rem;
		cursor: pointer;
		white-space: nowrap;
		transition: border-color 0.15s, background 0.15s;
	}

	.action-pill:hover {
		border-color: #555;
		background: #222;
	}

	.action-pill:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.open-pill.drag-over {
		border-color: #2563eb;
		background: #1a1a2e;
	}

	.lr-pill.active {
		border-color: #2563eb;
		color: white;
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
