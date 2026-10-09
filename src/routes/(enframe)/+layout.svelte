<script lang="ts">
// enframe's shell for every framer, library and editor screen: the shared look in enframe's blue, and the
// same top dock as Stabilize's, whose first island is the app switch.
import AppSwitch from '$lib/components/AppSwitch.svelte';
import { hasNativeShell } from '$lib/bridge';
import { provideDock } from '$lib/components/dock.svelte';

let { children } = $props();

const dock = provideDock();
</script>

<div class="app enframe" class:native={hasNativeShell()}>
	<div class="dock top">
		<div class="island start">
			<AppSwitch current="enframe" />
			{@render dock.parts.title?.()}
		</div>
		{@render dock.parts.islands?.()}
	</div>
	{@render children()}
</div>

<style>
	/* The tokens reach the screens without boxing them in: each enframe screen lays itself out on the page. */
	.enframe {
		display: contents;
	}

	/* Above Standalone's `.corner` (z-index 50), which reserves the corner for the native traffic lights. */
	.enframe > .dock {
		position: fixed;
		z-index: 60;
	}
</style>
