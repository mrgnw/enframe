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
