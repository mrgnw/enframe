<script lang="ts">
import { onMount } from 'svelte';
import { fade } from 'svelte/transition';

// The service worker broadcasts precache progress on this channel while it installs.
// No messages means nothing to show: an already-installed worker, the desktop shell,
// or a browser without workers all leave the pill unmounted.
const LINGER_MS = 2500;

let done = $state(0);
let total = $state(0);
let ready = $state(false);

let visible = $derived(total > 0);
let fraction = $derived(total > 0 ? done / total : 0);

onMount(() => {
	if (typeof BroadcastChannel === 'undefined') return;
	const channel = new BroadcastChannel('enframe-offline');
	let linger: ReturnType<typeof setTimeout> | undefined;
	channel.onmessage = (event: MessageEvent) => {
		const message = event.data as { type?: string; done?: number; total?: number };
		if (message.type === 'progress') {
			done = message.done ?? 0;
			total = message.total ?? 0;
		} else if (message.type === 'done') {
			ready = true;
			linger = setTimeout(() => {
				total = 0;
			}, LINGER_MS);
		}
	};
	return () => {
		channel.close();
		clearTimeout(linger);
	};
});
</script>

{#if visible}
	<div
		class="fixed right-3 bottom-3 z-50 flex items-center gap-2 rounded-full border bg-background/80 px-3 py-1.5 text-xs text-muted-foreground shadow-sm backdrop-blur"
		role="status"
		aria-live="polite"
		transition:fade={{ duration: 300 }}
	>
		{#if ready}
			<span>Available offline</span>
		{:else}
			<span>Preparing offline</span>
			<span class="h-1 w-16 overflow-hidden rounded-full bg-muted">
				<span class="block h-full rounded-full bg-foreground/60 transition-[width] duration-200" style="width: {Math.round(fraction * 100)}%"></span>
			</span>
		{/if}
	</div>
{/if}
