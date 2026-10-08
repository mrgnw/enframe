<script lang="ts">
	// `/preview?folder=PATH`: the clip playing from the folder's saved document, and the export panel.
	import { page } from '$app/state';
	import Preview from '#lib/preview/Preview.svelte';

	const folder = $derived(page.url.searchParams.get('folder'));
</script>

<svelte:head>
	<title>pinframe{folder ? ' · preview and export' : ''}</title>
</svelte:head>

{#if folder}
	{#key folder}
		<Preview {folder} />
	{/key}
{:else}
	<main class="stage">
		<section class="drop">
			<div>
				<h1>Which <b>folder</b>?</h1>
				<p>The preview plays a folder's clip once its points are marked.</p>
				<a class="btn primary" href="/">Choose a folder</a>
			</div>
		</section>
	</main>
	<nav class="filmstrip empty"></nav>
{/if}
