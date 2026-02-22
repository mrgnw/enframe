<script lang="ts">
import { onMount } from 'svelte';
import { importFile } from '$lib/db';
import {
	bestLocalPath,
	getLrAssetPaths,
	type LrAlbum,
	type LrAsset,
	type LrAssetPaths,
	listLrAlbumAssets,
	listLrAlbums,
	listLrRecent,
	searchLrAssets
} from '$lib/lightroom';
import { getImageSrc } from '$lib/tauri';

interface Props {
	onPhotosImported?: () => void;
}

let { onPhotosImported }: Props = $props();

let albums = $state<LrAlbum[]>([]);
let assets = $state<LrAsset[]>([]);
let pathsMap = $state<Record<string, LrAssetPaths>>({});
let selectedAlbum = $state<LrAlbum | null>(null);
let searchQuery = $state('');
let searchTimeout: ReturnType<typeof setTimeout> | null = null;
let loading = $state(false);
let loadingPaths = $state(false);
let error = $state<string | null>(null);
let importingIds = $state<Set<string>>(new Set());
let mode = $state<'recent' | 'album' | 'search'>('recent');

onMount(async () => {
	await loadAlbums();
	await loadRecent();
});

async function loadAlbums() {
	try {
		albums = await listLrAlbums();
	} catch (err) {
		console.error('Failed to load albums:', err);
	}
}

async function loadRecent() {
	loading = true;
	error = null;
	mode = 'recent';
	selectedAlbum = null;
	try {
		assets = await listLrRecent(200);
		await resolveAssetPaths();
	} catch (err) {
		error = err instanceof Error ? err.message : 'Failed to load recent photos';
	} finally {
		loading = false;
	}
}

async function loadAlbumAssets(album: LrAlbum) {
	loading = true;
	error = null;
	mode = 'album';
	selectedAlbum = album;
	searchQuery = '';
	try {
		assets = await listLrAlbumAssets(album.album_id, 500);
		await resolveAssetPaths();
	} catch (err) {
		error = err instanceof Error ? err.message : 'Failed to load album';
	} finally {
		loading = false;
	}
}

async function doSearch(query: string) {
	if (!query.trim()) {
		await loadRecent();
		return;
	}
	loading = true;
	error = null;
	mode = 'search';
	selectedAlbum = null;
	try {
		assets = await searchLrAssets(query, 200);
		await resolveAssetPaths();
	} catch (err) {
		error = err instanceof Error ? err.message : 'Search failed';
	} finally {
		loading = false;
	}
}

function handleSearchInput(e: Event) {
	const value = (e.target as HTMLInputElement).value;
	searchQuery = value;
	if (searchTimeout) clearTimeout(searchTimeout);
	searchTimeout = setTimeout(() => doSearch(value), 300);
}

async function resolveAssetPaths() {
	if (assets.length === 0) return;
	loadingPaths = true;
	try {
		const ids = assets.map((a) => a.asset_id);
		const batchSize = 50;
		const newMap: Record<string, LrAssetPaths> = {};
		for (let i = 0; i < ids.length; i += batchSize) {
			const batch = ids.slice(i, i + batchSize);
			const paths = await getLrAssetPaths(batch);
			for (const p of paths) {
				newMap[p.asset_id] = p;
			}
		}
		pathsMap = newMap;
	} catch (err) {
		console.error('Failed to resolve paths:', err);
	} finally {
		loadingPaths = false;
	}
}

function thumbnailSrc(asset: LrAsset): string | null {
	const paths = pathsMap[asset.asset_id];
	if (!paths) return null;
	const p = bestLocalPath(paths);
	return p ? getImageSrc(p) : null;
}

async function handleImport(asset: LrAsset) {
	const paths = pathsMap[asset.asset_id];
	if (!paths) return;
	const filePath = bestLocalPath(paths);
	if (!filePath) return;

	importingIds = new Set([...importingIds, asset.asset_id]);
	try {
		await importFile(filePath, {
			width: asset.width ?? undefined,
			height: asset.height ?? undefined,
			title: asset.filename ?? undefined,
			lr_asset_id: asset.asset_id
		});
		onPhotosImported?.();
	} catch (err) {
		console.error('Import failed:', err);
	} finally {
		const next = new Set(importingIds);
		next.delete(asset.asset_id);
		importingIds = next;
	}
}

function formatDate(d: string | null): string {
	if (!d) return '';
	try {
		return new Date(d).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
	} catch {
		return d;
	}
}

let heading = $derived(
	mode === 'search'
		? `Search: "${searchQuery}"`
		: mode === 'album' && selectedAlbum
			? selectedAlbum.name
			: 'Recent Photos'
);
</script>

<div class="lr-browser">
	<div class="sidebar">
		<div class="sidebar-header">Albums</div>
		<button class="album-item" class:active={mode === 'recent'} onclick={loadRecent}>
			Recent
		</button>
		{#each albums as album}
			<button
				class="album-item"
				class:active={selectedAlbum?.album_id === album.album_id}
				onclick={() => loadAlbumAssets(album)}
			>
				<span class="album-name">{album.name}</span>
				<span class="album-count">{album.asset_count}</span>
			</button>
		{/each}
	</div>

	<div class="main">
		<div class="toolbar">
			<div class="toolbar-left">
				<h2>{heading}</h2>
				{#if !loading}
					<span class="count">{assets.length} photos</span>
				{/if}
			</div>
			<input
				type="search"
				placeholder="Search by filename..."
				value={searchQuery}
				oninput={handleSearchInput}
				class="search-input"
			/>
		</div>

		{#if loading}
			<div class="status">Loading...</div>
		{:else if error}
			<div class="status error">{error}</div>
		{:else if assets.length === 0}
			<div class="status">No photos found</div>
		{:else}
			<div class="grid">
				{#each assets as asset (asset.asset_id)}
					{@const src = thumbnailSrc(asset)}
					{@const isImporting = importingIds.has(asset.asset_id)}
					{@const hasLocal = asset.has_local_original || asset.has_local_proxy}
					<div class="photo-item" class:cloud-only={!hasLocal}>
						<div class="thumbnail">
							{#if src}
								<img {src} alt={asset.filename ?? ''} loading="lazy" />
							{:else if loadingPaths}
								<div class="placeholder">...</div>
							{:else}
								<div class="placeholder">No preview</div>
							{/if}
							{#if asset.rating && asset.rating > 0}
								<span class="badge rating">{'*'.repeat(asset.rating)}</span>
							{/if}
						</div>
						<div class="info">
							<span class="filename" title={asset.filename ?? ''}>{asset.filename ?? 'Unknown'}</span>
							<span class="date">{formatDate(asset.capture_date)}</span>
						</div>
						<button
							class="import-btn"
							onclick={() => handleImport(asset)}
							disabled={isImporting || !hasLocal}
							title={hasLocal ? 'Import to Enframe' : 'No local file available'}
						>
							{isImporting ? 'Importing...' : hasLocal ? 'Import' : 'Cloud only'}
						</button>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>

<style>
	.lr-browser {
		display: flex;
		border: 1px solid #333;
		border-radius: 0.5rem;
		overflow: hidden;
		height: 500px;
		background: #1a1a1a;
	}

	.sidebar {
		width: 200px;
		min-width: 200px;
		border-right: 1px solid #333;
		overflow-y: auto;
		background: #111;
	}

	.sidebar-header {
		padding: 0.75rem 1rem;
		font-size: 0.7rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: #888;
		border-bottom: 1px solid #333;
	}

	.album-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		padding: 0.4rem 1rem;
		background: none;
		border: none;
		color: #ccc;
		font-size: 0.8rem;
		cursor: pointer;
		text-align: left;
	}

	.album-item:hover {
		background: #222;
	}

	.album-item.active {
		background: #2563eb;
		color: white;
	}

	.album-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		flex: 1;
	}

	.album-count {
		font-size: 0.7rem;
		color: #888;
		margin-left: 0.5rem;
		flex-shrink: 0;
	}

	.album-item.active .album-count {
		color: rgba(255, 255, 255, 0.7);
	}

	.main {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.toolbar {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.5rem 1rem;
		border-bottom: 1px solid #333;
		gap: 1rem;
		flex-shrink: 0;
	}

	.toolbar-left {
		display: flex;
		align-items: baseline;
		gap: 0.75rem;
		min-width: 0;
	}

	.toolbar h2 {
		margin: 0;
		font-size: 0.9rem;
		font-weight: 600;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.count {
		color: #888;
		font-size: 0.75rem;
		white-space: nowrap;
	}

	.search-input {
		padding: 0.3rem 0.6rem;
		background: #222;
		border: 1px solid #444;
		border-radius: 0.25rem;
		color: #ddd;
		font-size: 0.8rem;
		width: 200px;
		flex-shrink: 0;
	}

	.search-input:focus {
		outline: none;
		border-color: #2563eb;
	}

	.status {
		text-align: center;
		padding: 3rem;
		color: #888;
		font-size: 0.85rem;
	}

	.status.error {
		color: #ef4444;
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
		gap: 0.75rem;
		padding: 0.75rem;
		overflow-y: auto;
		flex: 1;
	}

	.photo-item {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.photo-item.cloud-only {
		opacity: 0.5;
	}

	.thumbnail {
		position: relative;
		aspect-ratio: 3/2;
		overflow: hidden;
		background: #222;
		border-radius: 0.25rem;
	}

	.thumbnail img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.placeholder {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		color: #555;
		font-size: 0.7rem;
	}

	.badge {
		position: absolute;
		bottom: 3px;
		right: 3px;
		font-size: 0.6rem;
		padding: 1px 4px;
		border-radius: 2px;
		background: rgba(0, 0, 0, 0.6);
		color: #fbbf24;
		font-weight: 700;
	}

	.info {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 0.25rem;
		padding: 0 0.15rem;
	}

	.filename {
		font-size: 0.7rem;
		color: #aaa;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.date {
		font-size: 0.6rem;
		color: #666;
		white-space: nowrap;
		flex-shrink: 0;
	}

	.import-btn {
		padding: 0.25rem 0.5rem;
		background: #2563eb;
		color: white;
		border: none;
		border-radius: 0.25rem;
		cursor: pointer;
		font-size: 0.7rem;
	}

	.import-btn:hover {
		background: #1d4ed8;
	}

	.import-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
		background: #555;
	}
</style>
