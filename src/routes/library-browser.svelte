<script lang="ts">
import { onMount } from 'svelte';
import { goto } from '$app/navigation';
import { invoke, isTauri } from '$lib/bridge';
import PhotoBrowser from '$lib/library/PhotoBrowser.svelte';

// ponytail: one probe stands in for the whole backend, the library UI needs all of it or none.
async function libraryReachable(): Promise<boolean> {
	if (isTauri()) return true;
	const response = await fetch('/sql/select', {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ sql: 'SELECT 1 FROM photos LIMIT 1', values: [] })
	}).catch(() => null);
	return response?.ok === true;
}

let reachable = $state<boolean | null>(null);

onMount(async () => {
	reachable = await libraryReachable();
	if (!reachable) return;
	const photoId = await invoke<number | null>('get_screenshot_mode').catch(() => null);
	if (photoId) goto(`/edit/${photoId}`);
});
</script>

{#if reachable === null}
	<p class="notice">Looking for a photo library…</p>
{:else if reachable}
	<PhotoBrowser />
{:else}
	<div class="notice">
		<p>No photo library is reachable from here.</p>
		<p><a href="/">Frame a single image instead</a></p>
	</div>
{/if}

<style>
	.notice {
		padding: 4rem 2rem;
		text-align: center;
		color: #6b7280;
	}

	.notice a {
		color: #60a5fa;
	}
</style>
