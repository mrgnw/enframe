<script lang="ts">
import { ChevronLeft, Folder, House, Image as ImageIcon } from '@lucide/svelte';
import { onMount } from 'svelte';
import { listDirectory, type LsEntry } from '$lib/bridge';

let {
	title,
	directory = false,
	multiple = false,
	extensions,
	onclose
}: {
	title?: string;
	directory?: boolean;
	multiple?: boolean;
	extensions?: string[];
	onclose: (selected: string[] | null) => void;
} = $props();

let path = $state('');
let parent = $state<string | null>(null);
let entries = $state<LsEntry[]>([]);
let selected = $state<string[]>([]);
let error = $state<string | null>(null);
let loading = $state(true);

const allowed = $derived(extensions ? new Set(extensions.map((e) => e.toLowerCase())) : null);

function isAllowed(entry: LsEntry): boolean {
	if (entry.isDirectory) return true;
	if (directory) return false;
	if (!allowed) return true;
	return allowed.has(entry.name.split('.').pop()?.toLowerCase() ?? '');
}

const visible = $derived(entries.filter(isAllowed));

async function load(dir?: string) {
	loading = true;
	error = null;
	try {
		const listing = await listDirectory(dir);
		path = listing.path;
		parent = listing.parent;
		entries = listing.entries;
		selected = [];
	} catch (e) {
		error = e instanceof Error ? e.message : String(e);
	} finally {
		loading = false;
	}
}

onMount(() => load());

function choose(entry: LsEntry) {
	if (entry.isDirectory) {
		load(entry.path);
		return;
	}
	if (!multiple) {
		onclose([entry.path]);
		return;
	}
	selected = selected.includes(entry.path) ? selected.filter((p) => p !== entry.path) : [...selected, entry.path];
}

function confirm() {
	onclose(directory ? [path] : selected);
}

function onkeydown(event: KeyboardEvent) {
	if (event.key === 'Escape') onclose(null);
}
</script>

<svelte:window {onkeydown} />

<div class="scrim">
	<div class="panel" role="dialog" aria-modal="true" aria-label={title ?? 'Choose'}>
		<div class="head">
			<h2>{title ?? (directory ? 'Choose folder' : 'Choose file')}</h2>
			<div class="nav">
				<button class="icon" onclick={() => load(parent ?? undefined)} disabled={!parent} title="Up one level">
					<ChevronLeft size={14} />
				</button>
				<button class="icon" onclick={() => load()} title="Home">
					<House size={14} />
				</button>
			</div>
		</div>

		<div class="path" title={path}>{path}</div>

		<div class="list">
			{#if loading}
				<p class="note">Loading…</p>
			{:else if error}
				<p class="note error">{error}</p>
			{:else if visible.length === 0}
				<p class="note">Nothing here</p>
			{:else}
				{#each visible as entry (entry.path)}
					<button
						class="entry"
						class:selected={selected.includes(entry.path)}
						onclick={() => choose(entry)}
					>
						{#if entry.isDirectory}
							<Folder size={13} />
						{:else if entry.isImage}
							<ImageIcon size={13} />
						{:else}
							<span class="spacer"></span>
						{/if}
						<span class="name">{entry.name}</span>
					</button>
				{/each}
			{/if}
		</div>

		<div class="foot">
			<button class="pill" onclick={() => onclose(null)}>Cancel</button>
			<button
				class="pill primary"
				onclick={confirm}
				disabled={!directory && selected.length === 0}
			>
				{directory ? 'Use this folder' : `Open${selected.length > 1 ? ` (${selected.length})` : ''}`}
			</button>
		</div>
	</div>
</div>

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 2000;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(0, 0, 0, 0.6);
		backdrop-filter: blur(2px);
	}

	.panel {
		display: flex;
		flex-direction: column;
		width: min(34rem, 92vw);
		height: min(30rem, 82vh);
		background: #1a1a1a;
		border: 1px solid #333;
		border-radius: 0.5rem;
		box-shadow: 0 1.5rem 3rem rgba(0, 0, 0, 0.5);
		overflow: hidden;
	}

	.head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.75rem 0.9rem 0.5rem;
	}

	.head h2 {
		margin: 0;
		font-size: 0.85rem;
		font-weight: 600;
		color: #eee;
	}

	.nav {
		display: flex;
		gap: 0.25rem;
	}

	.icon {
		display: flex;
		align-items: center;
		padding: 0.3rem;
		background: transparent;
		border: 1px solid #333;
		border-radius: 0.375rem;
		color: #ccc;
		cursor: pointer;
	}

	.icon:hover:not(:disabled) {
		border-color: #555;
		background: #222;
	}

	.icon:disabled {
		opacity: 0.35;
		cursor: not-allowed;
	}

	.path {
		padding: 0 0.9rem 0.6rem;
		font-size: 0.7rem;
		color: #6b7280;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		direction: rtl;
		text-align: left;
	}

	.list {
		flex: 1;
		overflow-y: auto;
		border-top: 1px solid #262626;
		border-bottom: 1px solid #262626;
		padding: 0.25rem;
	}

	.entry {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		width: 100%;
		padding: 0.35rem 0.5rem;
		background: transparent;
		border: none;
		border-radius: 0.3rem;
		color: #ccc;
		font-size: 0.78rem;
		text-align: left;
		cursor: pointer;
	}

	.entry:hover {
		background: #232323;
	}

	.entry.selected {
		background: #1a1a2e;
		color: white;
		box-shadow: inset 0 0 0 1px #2563eb;
	}

	.spacer {
		width: 13px;
	}

	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.note {
		margin: 0;
		padding: 1.5rem;
		text-align: center;
		font-size: 0.75rem;
		color: #6b7280;
	}

	.note.error {
		color: #f87171;
	}

	.foot {
		display: flex;
		justify-content: flex-end;
		gap: 0.5rem;
		padding: 0.6rem 0.9rem;
	}

	.pill {
		padding: 0.35rem 0.75rem;
		background: #1a1a1a;
		border: 1px solid #333;
		border-radius: 0.375rem;
		color: #ccc;
		font-size: 0.75rem;
		cursor: pointer;
		transition: border-color 0.15s, background 0.15s;
	}

	.pill:hover:not(:disabled) {
		border-color: #555;
		background: #222;
	}

	.pill.primary {
		border-color: #2563eb;
		color: white;
	}

	.pill:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
</style>
