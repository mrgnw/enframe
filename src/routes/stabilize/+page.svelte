<script lang="ts">
// `/stabilize?folder=PATH` opens the marking screen on that folder; `?trail=steps|flow|fade` picks the trail
// style. Without a folder: the picker of the connected instance.
import { goto } from '$app/navigation';
import { page } from '$app/state';
import { pickDirectory } from '$lib/bridge';
import Marking from '$lib/stabilize/Marking.svelte';
import { trailModeFrom } from '$lib/stabilize/trail';

const folder = $derived(page.url.searchParams.get('folder'));
const trailMode = $derived(trailModeFrom(page.url.searchParams.get('trail')));

async function choose() {
	const picked = await pickDirectory('Folder of photos');
	if (picked) await goto(`/stabilize?folder=${encodeURIComponent(picked)}`);
}
</script>

<svelte:head>
	<title>Stabilize{folder ? ' · fix lost photos' : ''}</title>
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
				<div class="actions">
					<button class="btn primary large" onclick={choose}>Open folder</button>
				</div>
				<small>Photos play in capture-time order · AVIF, HEIC, JPEG, PNG</small>
			</div>
		</section>
	</main>
{/if}
