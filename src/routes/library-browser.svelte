<script lang="ts">
import { onMount } from 'svelte';
import { goto } from '$app/navigation';
import { currentConnection, hasNativeShell, invoke, libraryReachable } from '$lib/bridge';
import ConnectionBar from '$lib/library/ConnectionBar.svelte';
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
		<p>No photo library answered at <code>{currentConnection() || 'this address'}</code>.</p>
		{#if hasNativeShell()}
			<p class="hint">Point this app at another instance:</p>
			<ConnectionBar />
		{/if}
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

	.notice code {
		color: #e4e4e7;
	}

	.notice .hint {
		font-size: 0.8rem;
		color: #71717a;
	}
</style>
