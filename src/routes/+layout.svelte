<script lang="ts">
import '../app.css';
import { Toaster } from '$lib/components/ui/sonner';
import { hasNativeShell } from '$lib/bridge';

let { children } = $props();

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

<Toaster />
