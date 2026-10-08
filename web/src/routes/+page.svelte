<script lang="ts">
	// `/?folder=PATH` opens the marking screen on that folder (absolute, or relative to the repository);
	// `?trail=steps|flow|fade` picks the trail style. Without a folder: the picker, a text field for the MVP.
	import { page } from '$app/state';
	import Marking from '#lib/marking/Marking.svelte';
	import { trailModeFrom } from '#lib/marking/trail.js';

	const folder = $derived(page.url.searchParams.get('folder'));
	const trailMode = $derived(trailModeFrom(page.url.searchParams.get('trail')));
	let typed = $state('');
</script>

<svelte:head>
	<title>pinframe{folder ? ' · fix lost photos' : ''}</title>
</svelte:head>

{#if folder}
	{#key folder}
		<Marking {folder} {trailMode} />
	{/key}
{:else}
	<main class="stage">
		<section class="drop">
			<div>
				<h1>Hold one thing <b>still</b></h1>
				<p>
					Open a folder of photos. Click points on the cloud, the mountain, the moon, whatever should stay
					put. Everything else moves around it.
				</p>
				<form method="get" action="/">
					<input name="folder" bind:value={typed} placeholder="/path/to/photos" aria-label="Folder of photos" />
					<button class="btn primary" type="submit" disabled={!typed.trim()}>Open folder</button>
				</form>
				<div class="sample">
					or try the <a href="/?folder={encodeURIComponent('samples/train clouds')}">train clouds sample</a> ·
					57 photos from a train
				</div>
				<small>Photos play in capture-time order · AVIF, HEIC, JPEG, PNG</small>
			</div>
		</section>
	</main>
	<nav class="filmstrip empty"></nav>
{/if}
