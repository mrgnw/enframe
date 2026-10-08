<script lang="ts">
// The clip: the current frame on a canvas the size of the preview frame, over the padding fill (a CSS
// checkerboard when transparent). The cross shows the pinned feature, which sits at the same spot in every frame.
import { pinAt } from './clip';
import type { Preview, View } from './preview-session.svelte';

let { session }: { session: Preview } = $props();

let canvas: HTMLCanvasElement | undefined = $state();

const view = $derived(session.view as View);
const pin = $derived(pinAt(view.rows[session.frame]));

$effect(() => {
	session.version;
	const { w, h } = view.size;
	const frame = session.bitmaps[session.frame];
	if (!canvas) return;
	if (canvas.width !== w) canvas.width = w;
	if (canvas.height !== h) canvas.height = h;
	const ctx = canvas.getContext('2d');
	if (!ctx) return;
	ctx.clearRect(0, 0, w, h);
	if (session.fill) {
		ctx.fillStyle = session.fill;
		ctx.fillRect(0, 0, w, h);
	}
	if (frame) ctx.drawImage(frame, 0, 0);
});
</script>

<div class="clip" class:checker={session.pad === 'transparent'} class:dim={session.busy && session.view} style:--r={view.size.w / view.size.h} id="clip">
	<canvas bind:this={canvas} aria-label="The clip, frame {session.frame + 1} of {view.rows.length}"></canvas>
	{#if pin}
		<span class="pin" style:left="{pin[0] * 100}%" style:top="{pin[1] * 100}%"></span>
	{/if}
</div>
