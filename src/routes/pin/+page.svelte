<script lang="ts">
// `/pin?folder=PATH` opens the marking screen on that folder; `?trail=steps|flow|fade` picks the trail
// style. Without a folder: the picker of the connected instance.
import { goto } from '$app/navigation';
import { page } from '$app/state';
import { pickDirectory } from '$lib/bridge';
import Marking from '$lib/pinframe/Marking.svelte';
import { trailModeFrom } from '$lib/pinframe/trail';

const folder = $derived(page.url.searchParams.get('folder'));
const trailMode = $derived(trailModeFrom(page.url.searchParams.get('trail')));

async function choose() {
	const picked = await pickDirectory('Folder of photos');
	if (picked) await goto(`/pin?folder=${encodeURIComponent(picked)}`);
}
</script>

<svelte:head>
	<title>pinframe{folder ? ' · fix lost photos' : ''}</title>
</svelte:head>

<div class="pinframe">
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
					<div class="actions">
						<button class="btn primary" onclick={choose}>Open folder</button>
					</div>
					<small>Photos play in capture-time order · AVIF, HEIC, JPEG, PNG</small>
				</div>
			</section>
		</main>
		<nav class="filmstrip empty"></nav>
	{/if}
</div>
