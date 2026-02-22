<script lang="ts">
import { Eye, FolderOpen, X } from '@lucide/svelte';
import { open } from '@tauri-apps/plugin-dialog';
import { onDestroy, onMount } from 'svelte';
import { importFile } from '$lib/db';
import { getImageSrc } from '$lib/tauri';
import { getWatchDir, scanWatchDir, setWatchDir } from '$lib/watch';

interface Props {
	onPhotosImported?: () => void;
}

let { onPhotosImported }: Props = $props();

let watchDir = $state<string | null>(getWatchDir());
let knownPaths = $state<Set<string>>(new Set());
let polling = $state(false);
let timer: ReturnType<typeof setInterval> | null = null;
let dragOver = $state(false);
let importing = $state(false);

onMount(() => {
	if (watchDir) startPolling();
});

onDestroy(() => {
	stopPolling();
});

function startPolling() {
	if (timer) return;
	polling = true;
	pollOnce();
	timer = setInterval(pollOnce, 3000);
}

function stopPolling() {
	if (timer) {
		clearInterval(timer);
		timer = null;
	}
	polling = false;
}

async function pollOnce() {
	if (!watchDir) return;
	try {
		const found = await scanWatchDir(watchDir);
		const newFiles = found.filter((f) => !knownPaths.has(f.path));
		if (newFiles.length > 0) {
			importing = true;
			for (const file of newFiles) {
				try {
					const src = getImageSrc(file.path);
					const dims = await loadImageDimensions(src);
					await importFile(file.path, { ...dims, title: file.filename });
					knownPaths = new Set([...knownPaths, file.path]);
				} catch (err) {
					console.error(`Auto-import failed: ${file.filename}`, err);
					knownPaths = new Set([...knownPaths, file.path]);
				}
			}
			importing = false;
			onPhotosImported?.();
		}
	} catch (err) {
		console.error('Watch scan failed:', err);
	}
}

function loadImageDimensions(src: string): Promise<{ width: number; height: number }> {
	return new Promise((resolve, reject) => {
		const img = new Image();
		img.onload = () => resolve({ width: img.naturalWidth, height: img.naturalHeight });
		img.onerror = reject;
		img.src = src;
	});
}

function handleDrop(e: DragEvent) {
	e.preventDefault();
	e.stopPropagation();
	dragOver = false;

	const items = e.dataTransfer?.items;
	if (!items) return;

	for (const item of Array.from(items)) {
		const entry = item.webkitGetAsEntry?.();
		if (entry?.isDirectory) {
			const path = (e.dataTransfer?.files[0] as any)?.path;
			if (path) {
				watchDir = path;
				setWatchDir(path);
				knownPaths = new Set();
				startPolling();
				return;
			}
		}
	}
}

function handleDragOver(e: DragEvent) {
	e.preventDefault();
	e.stopPropagation();
	dragOver = true;
}

function handleDragLeave(e: DragEvent) {
	e.preventDefault();
	dragOver = false;
}

async function pickWatchDir() {
	const dir = await open({ directory: true, title: 'Select folder to watch' });
	if (!dir || typeof dir !== 'string') return;
	watchDir = dir;
	setWatchDir(dir);
	knownPaths = new Set();
	startPolling();
}

function clearWatch() {
	stopPolling();
	watchDir = null;
	setWatchDir(null);
	knownPaths = new Set();
}

let dirName = $derived(watchDir?.split('/').pop() ?? '');
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_no_noninteractive_tabindex -->
<div
	class="watch-pill"
	class:active={!!watchDir}
	class:drag-over={dragOver}
	class:clickable={!watchDir}
	ondrop={handleDrop}
	ondragover={handleDragOver}
	ondragleave={handleDragLeave}
	onclick={() => { if (!watchDir) pickWatchDir(); }}
	role={watchDir ? undefined : 'button'}
	tabindex={watchDir ? undefined : 0}
>
	{#if watchDir}
		<Eye size={14} class="icon-watch" />
		<span class="dir-name" title={watchDir}>{dirName}</span>
		{#if importing}
			<span class="status importing">IMPORTING</span>
		{:else}
			<span class="status watching">WATCHING</span>
		{/if}
		<button class="clear-btn" onclick={clearWatch} title="Stop watching">
			<X size={12} />
		</button>
	{:else}
		<FolderOpen size={14} class="icon-inactive" />
		<span class="drop-hint">Watch folder</span>
	{/if}
</div>

<style>
	.watch-pill {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.35rem 0.65rem;
		background: #1a1a1a;
		border: 1px solid #333;
		border-radius: 0.375rem;
		font-size: 0.75rem;
		cursor: default;
		transition: border-color 0.15s, background 0.15s;
		white-space: nowrap;
	}

	.watch-pill.clickable {
		cursor: pointer;
	}

	.watch-pill.clickable:hover {
		border-color: #555;
		background: #222;
	}

	.watch-pill.drag-over {
		border-color: #4ade80;
		background: #1a2e1a;
	}

	.watch-pill.active {
		border-color: #444;
	}

	.watch-pill :global(.icon-watch) {
		color: #4ade80;
		flex-shrink: 0;
	}

	.watch-pill :global(.icon-inactive) {
		color: #555;
		flex-shrink: 0;
	}

	.dir-name {
		color: #999;
		max-width: 120px;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.status {
		font-size: 0.6rem;
		font-weight: 700;
		letter-spacing: 0.05em;
		text-transform: uppercase;
	}

	.status.watching {
		color: #4ade80;
	}

	.status.importing {
		color: #fbbf24;
	}

	.clear-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0.15rem;
		background: none;
		border: none;
		color: #666;
		cursor: pointer;
		border-radius: 0.2rem;
		flex-shrink: 0;
	}

	.clear-btn:hover {
		color: #ccc;
		background: #333;
	}

	.drop-hint {
		display: flex;
		align-items: center;
		gap: 0.3rem;
		color: #555;
		font-size: 0.7rem;
	}
</style>
