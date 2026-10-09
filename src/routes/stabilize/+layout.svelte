<script lang="ts">
// Stabilize's shell for every screen and every state, the folder picker and the loading and error screens
// included: the shared look in Stabilize's amber, and the top dock, whose first island is the app switch.
import AppSwitch from '$lib/components/AppSwitch.svelte';
import { hasNativeShell } from '$lib/bridge';
import { provideDock } from '$lib/components/dock.svelte';
import '$lib/stabilize/stabilize.css';
import '$lib/stabilize/marking.css';

let { children } = $props();

const dock = provideDock();
</script>

<div class="app stabilize" class:native={hasNativeShell()}>
	<div class="dock top">
		<div class="island start">
			<AppSwitch current="stabilize" responsive />
			{@render dock.parts.title?.()}
		</div>
		{@render dock.parts.islands?.()}
	</div>
	{@render children()}
</div>
