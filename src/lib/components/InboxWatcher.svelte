<script lang="ts">
import { open } from '@tauri-apps/plugin-dialog';
import { onDestroy, onMount } from 'svelte';
import { importFile } from '$lib/db';
import { getInboxDir, type InboxFile, scanInbox, setInboxDir } from '$lib/inbox';
import { getImageSrc } from '$lib/tauri';

interface Props {
	onPhotosImported?: () => void;
}

let { onPhotosImported }: Props = $props();

let inboxDir = $state<string | null>(getInboxDir());
let files = $state<InboxFile[]>([]);
let knownPaths = $state<Set<string>>(new Set());
let polling = $state(false);
let timer: ReturnType<typeof setInterval> | null = null;

onMount(() => {
	if (inboxDir) startPolling();
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
	if (!inboxDir) return;
	try {
		const found = await scanInbox(inboxDir);
		const newFiles = found.filter((f) => !knownPaths.has(f.path));
		if (newFiles.length > 0) {
			files = found;
		}
	} catch (err) {
		console.error('Inbox scan failed:', err);
	}
}

async function pickInboxDir() {
	const dir = await open({ directory: true, title: 'Select inbox folder' });
	if (!dir || typeof dir !== 'string') return;
	inboxDir = dir;
	setInboxDir(dir);
	knownPaths = new Set();
	files = [];
	startPolling();
	await pollOnce();
}

function clearInbox() {
	stopPolling();
	inboxDir = null;
	setInboxDir(null);
	files = [];
	knownPaths = new Set();
}

async function importAll() {
	const toImport = files.filter((f) => !knownPaths.has(f.path));
	for (const file of toImport) {
		try {
			const src = getImageSrc(file.path);
			const dims = await loadImageDimensions(src);
			await importFile(file.path, { ...dims, title: file.filename });
			knownPaths = new Set([...knownPaths, file.path]);
		} catch (err) {
			console.error(`Failed to import ${file.filename}:`, err);
		}
	}
	onPhotosImported?.();
}

async function importOne(file: InboxFile) {
	try {
		const src = getImageSrc(file.path);
		const dims = await loadImageDimensions(src);
		await importFile(file.path, { ...dims, title: file.filename });
		knownPaths = new Set([...knownPaths, file.path]);
		onPhotosImported?.();
	} catch (err) {
		console.error(`Failed to import ${file.filename}:`, err);
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

let newFiles = $derived(files.filter((f) => !knownPaths.has(f.path)));
</script>

<div class="inbox">
	<div class="inbox-header">
		<span class="inbox-title">Inbox</span>
		{#if inboxDir}
			<span class="inbox-dir" title={inboxDir}>{inboxDir.split('/').pop()}</span>
			<span class="inbox-status">{polling ? 'Watching' : 'Stopped'}</span>
		{/if}
		<div class="inbox-actions">
			{#if inboxDir}
				{#if newFiles.length > 0}
					<button class="btn-sm" onclick={importAll}>Import {newFiles.length}</button>
				{/if}
				<button class="btn-sm btn-muted" onclick={clearInbox}>Clear</button>
			{:else}
				<button class="btn-sm" onclick={pickInboxDir}>Set Inbox Folder</button>
			{/if}
		</div>
	</div>

	{#if inboxDir && newFiles.length > 0}
		<div class="inbox-files">
			{#each newFiles as file (file.path)}
				<div class="inbox-file">
					<span class="inbox-filename">{file.filename}</span>
					<button class="btn-xs" onclick={() => importOne(file)}>Import</button>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.inbox {
		border: 1px solid #333;
		border-radius: 0.375rem;
		background: #1a1a1a;
		margin-bottom: 1rem;
	}

	.inbox-header {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.5rem 0.75rem;
	}

	.inbox-title {
		font-size: 0.8rem;
		font-weight: 600;
		color: #ddd;
	}

	.inbox-dir {
		font-size: 0.7rem;
		color: #888;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		max-width: 150px;
	}

	.inbox-status {
		font-size: 0.65rem;
		color: #4ade80;
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.inbox-actions {
		margin-left: auto;
		display: flex;
		gap: 0.5rem;
	}

	.btn-sm {
		padding: 0.2rem 0.5rem;
		font-size: 0.7rem;
		background: #2563eb;
		color: white;
		border: none;
		border-radius: 0.25rem;
		cursor: pointer;
	}

	.btn-sm:hover {
		background: #1d4ed8;
	}

	.btn-muted {
		background: #333;
		color: #aaa;
	}

	.btn-muted:hover {
		background: #444;
	}

	.btn-xs {
		padding: 0.1rem 0.4rem;
		font-size: 0.65rem;
		background: #2563eb;
		color: white;
		border: none;
		border-radius: 0.2rem;
		cursor: pointer;
	}

	.inbox-files {
		border-top: 1px solid #333;
		max-height: 150px;
		overflow-y: auto;
	}

	.inbox-file {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.3rem 0.75rem;
		border-bottom: 1px solid #2a2a2a;
	}

	.inbox-file:last-child {
		border-bottom: none;
	}

	.inbox-filename {
		font-size: 0.7rem;
		color: #ccc;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
