<script lang="ts">
// enframe's shell for every framer, library and editor screen: the wordmark top-left is the app switch.
import AppSwitch from '$lib/components/AppSwitch.svelte';
import { hasNativeShell } from '$lib/bridge';

let { children } = $props();
</script>

<div class="app-switch-slot" class:native={hasNativeShell()}>
	<AppSwitch current="enframe" />
</div>

{@render children()}

<style>
	.app-switch-slot {
		position: fixed;
		top: 12px;
		left: 14px;
		/* Above Standalone's `.corner` (z-index 50), which reserves this corner for the native traffic lights. */
		z-index: 60;
		/* Every enframe screen is dark (#1a1a1a, the library #161616); the page's own text colour is the light theme's. */
		color: #ededed;
	}

	/* The native window's traffic lights sit in this corner on the desktop shell. */
	.app-switch-slot.native {
		left: 120px;
	}
</style>
