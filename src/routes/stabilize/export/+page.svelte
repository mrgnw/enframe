<script lang="ts">
// `/stabilize/export?folder=PATH&feature=NAME`: the feature's clip playing from the folder's saved document (the first
// feature without a name), and the export panel.
import { page } from '$app/state';
import Preview from '$lib/stabilize/Preview.svelte';

const folder = $derived(page.url.searchParams.get('folder'));
const feature = $derived(page.url.searchParams.get('feature'));
</script>

<svelte:head>
	<title>Stabilize{folder ? ' · preview and export' : ''}</title>
</svelte:head>

{#if folder}
	{#key folder}
		<Preview {folder} {feature} />
	{/key}
{:else}
	<main class="stage">
		<section class="drop">
			<div>
				<h1>Which <b>folder</b>?</h1>
				<p>The preview plays a folder's clip once its points are marked.</p>
				<a class="btn primary" href="/stabilize">Choose a folder</a>
			</div>
		</section>
	</main>
	<nav class="filmstrip empty"></nav>
{/if}
