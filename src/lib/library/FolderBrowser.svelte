<script lang="ts">
import { FolderPlus, X } from '@lucide/svelte';
import { onMount } from 'svelte';
import { toast } from 'svelte-sonner';
import { getImageSrc, invoke, hasNativeShell, pickDirectory } from '$lib/bridge';
import { importFile } from '$lib/db';
import { type FolderFile, getFolders, persistFolders, scanFolder } from '$lib/folders';
import { isRawName } from '$lib/framer/raw';

interface Props {
	onPhotosImported?: () => void;
}

let { onPhotosImported }: Props = $props();

let folders = $state<string[]>(getFolders());
let current = $state<string | null>(null);
let files = $state<FolderFile[]>([]);
let selected = $state<Set<string>>(new Set());
let loading = $state(false);
let importing = $state(false);
let error = $state<string | null>(null);
let scanRun = 0;

onMount(() => {
	if (folders.length > 0) selectFolder(folders[0]);
});

async function selectFolder(dir: string) {
	current = dir;
	selected = new Set();
	loading = true;
	error = null;
	const run = ++scanRun;
	try {
		const found = await scanFolder(dir);
		if (run !== scanRun) return;
		files = found;
		void developRawThumbnails(found, run);
	} catch (err) {
		if (run !== scanRun) return;
		files = [];
		error = err instanceof Error ? err.message : 'Failed to read folder';
	} finally {
		if (run === scanRun) loading = false;
	}
}

export function openFolder(dir: string) {
	if (!folders.includes(dir)) {
		folders = [...folders, dir];
		persistFolders(folders);
	}
	selectFolder(dir);
}

async function addFolder() {
	const dir = await pickDirectory('Select folder to bookmark');
	if (dir) openFolder(dir);
}

function removeFolder(dir: string) {
	folders = folders.filter((f) => f !== dir);
	persistFolders(folders);
	if (current === dir) {
		if (folders.length > 0) {
			selectFolder(folders[0]);
		} else {
			scanRun++;
			current = null;
			files = [];
			selected = new Set();
		}
	}
}

// ponytail: same worker queue as LightroomBrowser's, left separate so a change
// here cannot re-break the verified Lightroom tiles; unify if a third copy appears.
const RAW_THUMB_WORKERS = 3;
let rawThumbs = $state<Record<string, string>>({});

async function developRawThumbnails(batch: FolderFile[], run: number) {
	if (!hasNativeShell()) return;
	const queue = batch.filter((f) => isRawName(f.path) && !(f.path in rawThumbs));
	await Promise.all(
		Array.from({ length: RAW_THUMB_WORKERS }, async () => {
			while (queue.length > 0 && run === scanRun) {
				const next = queue.shift();
				if (!next) return;
				const jpeg = await invoke<string | null>('raw_thumbnail', { path: next.path }).catch(() => null);
				if (jpeg) rawThumbs[next.path] = `data:image/jpeg;base64,${jpeg}`;
			}
		})
	);
}

function thumbnailSrc(file: FolderFile): string | null {
	if (isRawName(file.path)) return rawThumbs[file.path] ?? null;
	return getImageSrc(file.path);
}

function toggleSelected(path: string) {
	const next = new Set(selected);
	if (next.has(path)) {
		next.delete(path);
	} else {
		next.add(path);
	}
	selected = next;
}

function selectAll() {
	selected = new Set(files.map((f) => f.path));
}

function loadImageDimensions(src: string): Promise<{ width: number; height: number }> {
	return new Promise((resolve, reject) => {
		const img = new Image();
		img.onload = () => resolve({ width: img.naturalWidth, height: img.naturalHeight });
		img.onerror = reject;
		img.src = src;
	});
}

async function importSelected() {
	const picked = files.filter((f) => selected.has(f.path));
	if (picked.length === 0) return;
	importing = true;
	let failed = 0;
	for (const file of picked) {
		try {
			const dims = await loadImageDimensions(getImageSrc(file.path)).catch(() => null);
			await importFile(file.path, { ...dims, title: file.filename });
		} catch (err) {
			failed++;
			console.error(`Import failed: ${file.filename}`, err);
		}
	}
	importing = false;
	if (failed > 0) toast.error(`${failed} of ${picked.length} imports failed`);
	if (failed < picked.length) {
		selected = new Set();
		onPhotosImported?.();
	}
}

function folderName(dir: string): string {
	return dir.split('/').pop() ?? dir;
}
</script>

<div class="folder-browser">
	<nav class="rail">
		<h3>Folders</h3>
		{#each folders as folder (folder)}
			<div class="folder-item" class:active={current === folder}>
				<button class="folder-name" title={folder} onclick={() => selectFolder(folder)}>
					{folderName(folder)}
				</button>
				<button class="remove-btn" title="Remove bookmark" onclick={() => removeFolder(folder)}>
					<X size={12} />
				</button>
			</div>
		{/each}
		<button class="add-folder" onclick={addFolder}>
			<FolderPlus size={14} />
			Add folder
		</button>
	</nav>

	<div class="main">
		{#if current}
			<div class="toolbar">
				<h2 title={current}>{folderName(current)}</h2>
				{#if !loading}
					<span class="count">{files.length} images</span>
				{/if}
				<div class="toolbar-actions">
					{#if selected.size < files.length && files.length > 0}
						<button class="toolbar-btn" onclick={selectAll}>Select all</button>
					{/if}
					{#if selected.size > 0}
						<button class="toolbar-btn" onclick={() => { selected = new Set(); }}>Clear</button>
						<button class="import-btn" onclick={importSelected} disabled={importing}>
							{importing ? 'Importing…' : `Import ${selected.size}`}
						</button>
					{/if}
				</div>
			</div>

			{#if loading}
				<div class="status">Loading…</div>
			{:else if error}
				<div class="status error">{error}</div>
			{:else if files.length === 0}
				<div class="status">No images in this folder</div>
			{:else}
				<div class="grid">
					{#each files as file (file.path)}
						{@const src = thumbnailSrc(file)}
						<button
							class="photo-item"
							class:selected={selected.has(file.path)}
							onclick={() => toggleSelected(file.path)}
						>
							<div class="thumbnail">
								{#if src}
									<img {src} alt={file.filename} loading="lazy" />
								{:else}
									<div class="placeholder">RAW</div>
								{/if}
								{#if selected.has(file.path)}
									<span class="check">✓</span>
								{/if}
							</div>
							<span class="filename" title={file.filename}>{file.filename}</span>
						</button>
					{/each}
				</div>
			{/if}
		{:else}
			<div class="status">
				<p>Bookmark a folder to browse its images.</p>
				<p class="hint">Nothing imports until you select images and click Import.</p>
			</div>
		{/if}
	</div>
</div>

<style>
	.folder-browser {
		flex: 1;
		display: flex;
		min-height: 0;
		overflow: hidden;
	}

	.rail {
		width: 208px;
		flex: none;
		display: flex;
		flex-direction: column;
		gap: 1px;
		padding: 0 0.625rem 1rem 1rem;
		overflow-y: auto;
	}

	.rail h3 {
		margin: 0;
		padding: 0.25rem 0.5rem 0.4rem;
		font-size: 0.66rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: #71717a;
	}

	.folder-item {
		display: flex;
		align-items: center;
		border-radius: 7px;
	}

	.folder-item:hover {
		background: #1e1e1e;
	}

	.folder-item.active {
		background: #2563eb;
	}

	.folder-name {
		flex: 1;
		padding: 0.375rem 0.55rem;
		background: none;
		border: none;
		color: #a1a1aa;
		font-size: 0.8rem;
		cursor: pointer;
		text-align: left;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.folder-item:hover .folder-name {
		color: #e4e4e7;
	}

	.folder-item.active .folder-name {
		color: white;
	}

	.remove-btn {
		display: none;
		align-items: center;
		padding: 0.25rem 0.4rem;
		background: none;
		border: none;
		color: #71717a;
		cursor: pointer;
	}

	.folder-item:hover .remove-btn {
		display: flex;
	}

	.folder-item.active .remove-btn {
		color: rgba(255, 255, 255, 0.7);
	}

	.add-folder {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		margin-top: 0.25rem;
		padding: 0.375rem 0.55rem;
		background: none;
		border: 1px dashed #333;
		border-radius: 7px;
		color: #71717a;
		font-size: 0.8rem;
		cursor: pointer;
	}

	.add-folder:hover {
		color: #e4e4e7;
		border-color: #555;
	}

	.main {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.toolbar {
		display: flex;
		align-items: baseline;
		padding: 0 1.25rem 0.5rem;
		gap: 0.75rem;
		min-width: 0;
		flex-shrink: 0;
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

	.toolbar-actions {
		display: flex;
		gap: 0.5rem;
		margin-left: auto;
	}

	.toolbar-btn {
		padding: 0.25rem 0.6rem;
		background: #1e1e1e;
		border: 1px solid #2e2e2e;
		border-radius: 6px;
		color: #a1a1aa;
		font-size: 0.75rem;
		cursor: pointer;
	}

	.toolbar-btn:hover {
		color: #e4e4e7;
		border-color: #555;
	}

	.import-btn {
		padding: 0.25rem 0.6rem;
		background: #2563eb;
		color: white;
		border: none;
		border-radius: 6px;
		cursor: pointer;
		font-size: 0.75rem;
	}

	.import-btn:hover {
		background: #1d4ed8;
	}

	.import-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.status {
		text-align: center;
		padding: 3rem;
		color: #888;
		font-size: 0.85rem;
	}

	.status .hint {
		color: #666;
		font-size: 0.75rem;
	}

	.status.error {
		color: #ef4444;
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
		align-content: start;
		gap: 0.875rem;
		padding: 0 1.25rem 1.25rem;
		overflow-y: auto;
		flex: 1;
	}

	.photo-item {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		padding: 0;
		background: none;
		border: none;
		cursor: pointer;
		text-align: left;
	}

	.thumbnail {
		position: relative;
		aspect-ratio: 3/2;
		overflow: hidden;
		background: #262626;
		border-radius: 10px;
		border: 2px solid transparent;
	}

	.photo-item.selected .thumbnail {
		border-color: #2563eb;
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

	.check {
		position: absolute;
		top: 6px;
		left: 6px;
		width: 18px;
		height: 18px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: #2563eb;
		color: white;
		border-radius: 50%;
		font-size: 0.65rem;
		font-weight: 700;
	}

	.filename {
		font-size: 0.7rem;
		color: #aaa;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
