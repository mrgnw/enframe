<script lang="ts">
import { page } from '$app/state';
import '$lib/pinframe/pin.css';
import '$lib/pinframe/marking.css';

const folder = $derived(page.url.searchParams.get('folder') ?? '');
</script>

<svelte:head>
	<title>pinframe · export</title>
</svelte:head>

<div class="pinframe">
	<main class="stage">
		<section class="drop">
			<div>
				<h1>Export is <b>next</b></h1>
				<p>The preview and export step is not built yet.</p>
				<div class="actions">
					<a class="btn primary" href="/pin?folder={encodeURIComponent(folder)}">Back to fixing</a>
				</div>
			</div>
		</section>
	</main>
	<nav class="filmstrip empty"></nav>
</div>
