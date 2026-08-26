<script lang="ts">
import { onMount } from 'svelte';
import { goto } from '$app/navigation';
import { invoke, libraryReachable } from '$lib/bridge';
import PhotoBrowser from '$lib/library/PhotoBrowser.svelte';

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
	/* ponytail: every surface paints its own dark, the shared body default is still light. */
	:global(body) {
		background: #161616;
	}

	.notice {
		padding: 4rem 2rem;
		text-align: center;
		color: #a1a1aa;
	}

	.notice a {
		color: #60a5fa;
	}
</style>
