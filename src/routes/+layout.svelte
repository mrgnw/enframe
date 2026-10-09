<script lang="ts">
import '../app.css';
import '$lib/theme.css';
import { onMount } from 'svelte';
import { dev } from '$app/environment';
import { onNavigate } from '$app/navigation';
import OfflineProgress from '$lib/components/OfflineProgress.svelte';
import { Toaster } from '$lib/components/ui/sonner';
import { hasNativeShell } from '$lib/bridge';

let { children } = $props();

// The browser build is the one that leaves the machine that owns it, so it is the one
// that earns an offline shell. The desktop shell's assets are already on disk.
onMount(() => {
	if (dev || hasNativeShell() || !('serviceWorker' in navigator)) return;
	navigator.serviceWorker.register('/service-worker.js').catch((error) => {
		console.warn('service worker registration failed:', error);
	});
});

const appOf = (routeId: string | null | undefined) => (routeId?.startsWith('/stabilize') ? 'stabilize' : 'enframe');

// Crossing between the apps cross-fades instead of cutting, and the app switch, named in its own styles, holds
// still through it. Moves within one app stay instant.
onNavigate((navigation) => {
	if (!document.startViewTransition) return;
	if (appOf(navigation.from?.route.id) === appOf(navigation.to?.route.id)) return;
	return new Promise((resolve) => {
		document.startViewTransition(async () => {
			resolve();
			await navigation.complete;
		});
	});
});

// The WebView's own Back/Reload menu is meaningless in a desktop app and lands on top of
// the framer. Text fields keep theirs, so copy and paste still work while editing labels.
function suppressWebviewMenu(event: MouseEvent) {
	if (!hasNativeShell()) return;
	const element = event.target as HTMLElement | null;
	const tag = element?.tagName.toLowerCase();
	if (tag === 'input' || tag === 'textarea' || element?.isContentEditable) return;
	event.preventDefault();
}
</script>

<svelte:document oncontextmenu={suppressWebviewMenu} />

{#if children}
    {@render children()}
{:else}
    <p>Fallback content</p>
{/if}

<OfflineProgress />
<Toaster />
