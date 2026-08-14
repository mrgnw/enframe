<script lang="ts">
import { Crop, FolderPlus, ImageOff, Link } from '@lucide/svelte';
import { onDestroy, onMount } from 'svelte';
import { toast } from 'svelte-sonner';
import { getImageSrc, onDragDrop, pickFiles, stat } from '$lib/bridge';
import {
	type Frame,
	importFile,
	importFromMetadata,
	listAllFrames,
	listPhotos,
	type Photo,
	updatePhoto
} from '$lib/db';
import { DESTINATIONS, REQUIRED_DESTINATIONS } from '$lib/destinations';
import { getWatchDir, persistWatchDir } from '$lib/watch';
import LightroomBrowser from './LightroomBrowser.svelte';
import WatchDir from './WatchDir.svelte';

const IMAGE_EXTS = new Set([
	'jpg',
	'jpeg',
	'png',
	'tiff',
	'tif',
	'heic',
	'heif',
	'webp',
	'avif',
	'jxl',
	'dng',
	'arw',
	'cr2',
	'cr3',
	'nef',
	'raf'
]);

function isImagePath(path: string): boolean {
	const ext = path.split('.').pop()?.toLowerCase() ?? '';
	return IMAGE_EXTS.has(ext);
}

function loadImageDimensions(src: string): Promise<{ width: number; height: number }> {
	return new Promise((resolve, reject) => {
		const img = new Image();
		img.onload = () => resolve({ width: img.naturalWidth, height: img.naturalHeight });
		img.onerror = reject;
		img.src = src;
	});
}

type Tab = 'photos' | 'lightroom' | 'watch';

const TAB_KEY = 'enframe.library.tab';
const TABS: { id: Tab; label: string }[] = [
	{ id: 'photos', label: 'Photos' },
	{ id: 'lightroom', label: 'Lightroom' },
	{ id: 'watch', label: 'Watch folder' }
];

function storedTab(): Tab {
	const saved = typeof localStorage === 'undefined' ? null : localStorage.getItem(TAB_KEY);
	return TABS.some((t) => t.id === saved) ? (saved as Tab) : 'photos';
}

function selectTab(next: Tab) {
	tab = next;
	if (next === 'lightroom') lrQuery = '';
	try {
		localStorage.setItem(TAB_KEY, next);
	} catch {}
}

let photos = $state<Photo[]>([]);
let framesByPhoto = $state<Record<number, Frame[]>>({});
let loading = $state(true);
let importing = $state(false);
let tab = $state<Tab>(storedTab());
let lrQuery = $state('');
let lightroom = $state<LightroomBrowser | null>(null);
let dragging = $state(false);
let draggingDir = $state(false);
let watchDir = $state<string | null>(getWatchDir());
let missingImages = $state<Set<number>>(new Set());
let unlisten: (() => void) | null = null;

function handleImageError(id: number) {
	missingImages = new Set(missingImages).add(id);
}

async function handleLinkImage(id: number) {
	const selected = await pickFiles({ extensions: Array.from(IMAGE_EXTS) });
	if (!selected) return;

	await updatePhotoAndReload(id, selected[0]);
}

async function updatePhotoAndReload(id: number, filePath: string) {
	const src = getImageSrc(filePath);
	try {
		const dims = await loadImageDimensions(src);
		await updatePhoto(id, {
			file_path: filePath,
			width: dims.width,
			height: dims.height
		});

		// Remove from missing set
		const newMissing = new Set(missingImages);
		newMissing.delete(id);
		missingImages = newMissing;

		await loadPhotos();
	} catch (e) {
		console.error('Failed to link image:', e);
		toast.error('Failed to link image');
	}
}

onMount(async () => {
	await loadPhotos();

	unlisten = await onDragDrop(async (payload) => {
		if (payload.type === 'enter') {
			dragging = true;
			const paths = payload.paths;
			if (paths.length === 1) {
				try {
					const info = await stat(paths[0]);
					draggingDir = info.isDirectory;
				} catch {
					draggingDir = false;
				}
			} else {
				draggingDir = false;
			}
		} else if (payload.type === 'leave') {
			dragging = false;
			draggingDir = false;
		} else if (payload.type === 'drop') {
			const paths = payload.paths;
			const position = payload.position;
			dragging = false;
			draggingDir = false;

			// Handle folder drop (watch dir)
			if (paths.length === 1) {
				try {
					const info = await stat(paths[0]);
					if (info.isDirectory) {
						watchDir = paths[0];
						persistWatchDir(paths[0]);
						return;
					}
				} catch {}
			}

			// Handle replacement drop on a missing image card
			if (paths.length === 1 && isImagePath(paths[0])) {
				// Check if dropped on a card with missing image
				// We need to find the element at the drop position
				// Since we can't easily access DOM elements by position here in a robust way without
				// potentially querying the DOM, let's try a different approach:
				// If we have missing images, we can check if the mouse is over one of them?
				// But we are in an async callback.
				// However, `document.elementFromPoint` should work if the window is focused.

				const el = document.elementFromPoint(position.x, position.y);
				const card = el?.closest('.photo-card[data-missing="true"]');
				if (card) {
					const idStr = card.getAttribute('data-id');
					if (idStr) {
						const id = parseInt(idStr, 10);
						await updatePhotoAndReload(id, paths[0]);
						return;
					}
				}
			}

			const imagePaths = paths.filter(isImagePath);
			const metadataPaths = paths.filter((p) => p.endsWith('.metadata.json'));

			if (metadataPaths.length > 0) {
				selectTab('photos');
				importing = true;
				try {
					for (const path of metadataPaths) {
						await importFromMetadata(path);
					}
					await loadPhotos();
				} catch (e) {
					console.error('Failed to import metadata:', e);
					toast.error('Failed to import metadata');
				} finally {
					importing = false;
				}
			}

			if (imagePaths.length > 0) {
				await importFiles(imagePaths);
			}
		}
	});
});

onDestroy(() => {
	unlisten?.();
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
		console.error('[PhotoBrowser] loadPhotos() FAILED:', error);
	} finally {
		loading = false;
	}
}

async function importFiles(paths: string[]) {
	selectTab('photos');
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
		toast.error('Failed to import files');
	} finally {
		importing = false;
	}
}

async function handleOpenClick() {
	const selected = await pickFiles({
		multiple: true,
		extensions: ['jpg', 'jpeg', 'png', 'jxl', 'tiff', 'tif', 'webp', 'heic', 'dng', 'arw']
	});
	if (!selected) return;
	await importFiles(selected);
}
</script>

<div
	class="photo-browser"
	class:drag-active={dragging && !draggingDir}
	role="application"
>
	<div class="appbar" data-tauri-drag-region>
		<span class="wordmark">Enframe</span>

		<nav class="tabs">
			{#each TABS as t (t.id)}
				<button class="tab" class:active={tab === t.id} onclick={() => selectTab(t.id)}>
					{t.label}
					{#if t.id === 'photos' && photos.length > 0}
						<span class="count">{photos.length}</span>
					{/if}
				</button>
			{/each}
		</nav>

		<div class="actions">
			{#if tab === 'lightroom'}
				<input
					type="search"
					class="search"
					placeholder="Search by filename…"
					value={lrQuery}
					oninput={(e) => {
						lrQuery = e.currentTarget.value;
						lightroom?.search(lrQuery);
					}}
				/>
			{:else}
				<button
					class="action-pill open-pill"
					class:drag-over={dragging && !draggingDir}
					onclick={handleOpenClick}
					disabled={importing}
				>
					<FolderPlus size={14} />
					{importing ? 'Importing...' : 'Open'}
				</button>

				<a class="action-pill" href="/" title="Frame one image without importing it">
					<Crop size={14} />
					Single
				</a>
			{/if}
		</div>
	</div>

	<!-- ponytail: the watch surface stays mounted on every tab so its poll keeps importing. -->
	<div class="panel watch-panel" class:hidden={tab !== 'watch'}>
		<WatchDir
			{watchDir}
			onWatchDirChange={(dir) => { watchDir = dir; }}
			onPhotosImported={loadPhotos}
			highlighted={dragging && draggingDir}
		/>
		<p class="hint">
			{watchDir ? 'New images landing here import themselves.' : 'Pick a folder — or drop one anywhere — and its new images import themselves.'}
		</p>
	</div>

	{#if tab === 'lightroom'}
		<LightroomBrowser bind:this={lightroom} onPhotosImported={loadPhotos} />
	{/if}

	{#if tab === 'photos'}
		{#if loading}
			<div class="loading">Loading photos...</div>
		{:else if importing}
			<div class="loading">Importing files...</div>
		{:else if photos.length === 0}
			<div class="empty">
				<p>No photos yet</p>
				<p class="hint">Drop images here, click Open, or browse Lightroom</p>
				<p class="hint">Just one image? <a href="/">Frame it without importing</a></p>
			</div>
		{:else}
			<div class="grid">
				{#each photos as photo (photo.id)}
					{@const covered = new Set((framesByPhoto[photo.id] ?? []).map((f) => f.destination))}
					<div
						class="photo-card"
						data-id={photo.id}
						data-missing={missingImages.has(photo.id)}
					>
						<a href="/edit/{photo.id}" class="photo-link" class:missing={missingImages.has(photo.id)}>
							<div class="thumbnail">
								{#if missingImages.has(photo.id)}
									<div class="missing-placeholder">
										<ImageOff size={32} />
										<span class="missing-text">Image not found</span>
										<span class="missing-path" title={photo.file_path}>
											{photo.file_path.split('/').pop()}
										</span>
										<button 
											class="link-btn"
											onclick={(e) => {
												e.preventDefault();
												e.stopPropagation();
												handleLinkImage(photo.id);
											}}
										>
											<Link size={12} />
											Link Image
										</button>
									</div>
								{:else}
									<img 
										src={getImageSrc(photo.file_path)} 
										alt={photo.title || 'Photo'} 
										onerror={() => {
											missingImages.add(photo.id);
											missingImages = new Set(missingImages);
										}}
									/>
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
								{/if}
								<div class="dest-badges">
									{#each DESTINATIONS as destination (destination.slug)}
										{@const isCovered = covered.has(destination.slug)}
										<span
											class="dest-dot"
											class:required={REQUIRED_DESTINATIONS.includes(destination.slug)}
											class:covered={isCovered}
											title="{destination.name}{isCovered ? '' : ' — not covered'}"
										></span>
									{/each}
								</div>
							</div>
							{#if photo.title}
								<div class="title">{photo.title}</div>
							{/if}
						</a>
					</div>
				{/each}
			</div>
		{/if}
	{/if}

	{#if dragging}
		<div class="drop-overlay">
			{#if draggingDir}
				<span class="drop-label">Drop to set watch folder</span>
			{:else}
				<span class="drop-label">Drop to import</span>
			{/if}
		</div>
	{/if}
</div>

<style>
	.photo-browser {
		position: relative;
		display: flex;
		flex-direction: column;
		height: 100vh;
		overflow: hidden;
		background: #161616;
		color: #e4e4e7;
	}

	.photo-browser.drag-active {
		outline: 2px dashed #2563eb;
		outline-offset: -2px;
	}

	.appbar {
		display: flex;
		align-items: center;
		gap: 1.25rem;
		padding: 0.75rem 1.25rem 0.75rem 5.5rem;
		flex: none;
	}

	.wordmark {
		font-weight: 600;
		font-size: 0.95rem;
	}

	.tabs {
		display: flex;
		gap: 2px;
		padding: 3px;
		background: #1e1e1e;
		border: 1px solid #2e2e2e;
		border-radius: 9px;
	}

	.tab {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		padding: 0.3rem 0.9rem;
		background: none;
		border: none;
		border-radius: 7px;
		color: #a1a1aa;
		font-size: 0.8rem;
		cursor: pointer;
	}

	.tab:hover {
		color: #d4d4d8;
	}

	.tab.active {
		background: #262626;
		color: #fff;
	}

	.tab .count {
		font-size: 0.72rem;
		color: #71717a;
	}

	.tab.active .count {
		color: rgba(255, 255, 255, 0.6);
	}

	.actions {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		margin-left: auto;
	}

	.search {
		width: 200px;
		padding: 0.35rem 0.75rem;
		background: #1e1e1e;
		border: 1px solid #2e2e2e;
		border-radius: 8px;
		color: #e4e4e7;
		font-size: 0.8rem;
	}

	.search:focus {
		outline: none;
		border-color: #2563eb;
	}

	.panel {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 0.75rem;
		min-height: 0;
	}

	.panel.hidden {
		display: none;
	}

	.watch-panel :global(.watch-pill) {
		font-size: 0.9rem;
		padding: 0.6rem 1rem;
		border-radius: 8px;
	}

	.watch-panel :global(.drop-hint) {
		font-size: 0.9rem;
		color: #a1a1aa;
	}

	.watch-panel .hint {
		margin: 0;
		color: #71717a;
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
		text-decoration: none;
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

	.drop-overlay {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(0, 0, 0, 0.6);
		z-index: 1000;
		pointer-events: none;
	}

	.drop-label {
		font-size: 1.25rem;
		font-weight: 600;
		color: white;
		padding: 1rem 2rem;
		background: rgba(37, 99, 235, 0.8);
		border-radius: 0.75rem;
		border: 2px dashed rgba(255, 255, 255, 0.4);
	}

	.loading,
	.empty {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		color: #71717a;
	}

	.hint {
		font-size: 0.875rem;
		margin-top: 0.5rem;
	}

	.hint a {
		color: #60a5fa;
		text-decoration: none;
	}

	.hint a:hover {
		text-decoration: underline;
	}

	.grid {
		flex: 1;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
		align-content: start;
		gap: 1rem;
		padding: 0.5rem 1.25rem 1.5rem;
		overflow-y: auto;
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

	.photo-link {
		display: block;
		width: 100%;
		height: 100%;
		text-decoration: none;
		color: inherit;
	}
	
	.photo-link.missing {
		cursor: default;
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

	.missing-placeholder {
		position: absolute;
		inset: 0;
		background: #2a2a2a;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		color: #888;
		padding: 1rem;
		text-align: center;
		border: 1px dashed #444;
	}

	.missing-text {
		font-size: 0.8rem;
		font-weight: 500;
	}

	.missing-path {
		font-size: 0.7rem;
		color: #666;
		word-break: break-all;
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.link-btn {
		margin-top: 0.5rem;
		display: flex;
		align-items: center;
		gap: 0.3rem;
		padding: 0.3rem 0.6rem;
		background: #333;
		border: 1px solid #444;
		border-radius: 4px;
		color: #ccc;
		font-size: 0.7rem;
		cursor: pointer;
		transition: all 0.2s;
	}

	.link-btn:hover {
		background: #444;
		border-color: #555;
		color: white;
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

	.dest-badges {
		position: absolute;
		right: 6px;
		bottom: 6px;
		display: flex;
		gap: 3px;
	}

	.dest-dot {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: rgba(255, 255, 255, 0.14);
	}

	.dest-dot.required {
		background: transparent;
		box-shadow: inset 0 0 0 1px rgba(59, 130, 246, 0.55);
	}

	.dest-dot.covered {
		background: #2563eb;
		box-shadow: none;
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
