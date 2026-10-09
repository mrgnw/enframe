<script lang="ts">
// Stabilize's shell for every screen and every state, the folder picker and the loading and error screens
// included: the dark theme and the top dock, whose first island is the app switch.
import AppSwitch from '$lib/components/AppSwitch.svelte';
import { provideDock } from '$lib/stabilize/dock.svelte';
import '$lib/stabilize/stabilize.css';
import '$lib/stabilize/marking.css';

let { children } = $props();

const dock = provideDock();
</script>

<!-- Its own stylesheet, not an @import in stabilize.css: a sheet is not applied until its imports load, so the
     shell would show unstyled for as long as Google Fonts takes, or for good offline. -->
<svelte:head>
	<link rel="preconnect" href="https://fonts.googleapis.com" />
	<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
	<link
		rel="stylesheet"
		href="https://fonts.googleapis.com/css2?family=Bricolage+Grotesque:opsz,wght@12..96,500;12..96,600&family=Karla:wght@400;500;600&display=swap"
	/>
</svelte:head>

<div class="stabilize">
	<div class="dock top">
		<div class="island start">
			<AppSwitch current="stabilize" responsive />
			{@render dock.parts.title?.()}
		</div>
		{@render dock.parts.islands?.()}
	</div>
	{@render children()}
</div>
