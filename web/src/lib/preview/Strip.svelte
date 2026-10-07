<script lang="ts">
	// The preview's filmstrip: the same thumbnails as the marking screen, one per photo, with the playing
	// frame lifted. Press and drag to scrub the clip; near either edge it scrolls on its own.
	import { untrack } from 'svelte';
	import { photoUrl } from './api';
	import { thumbnail } from '#lib/marking/thumbs.js';
	import { clamp } from '#lib/marking/types.js';
	import type { Preview } from './session.svelte';

	let { session }: { session: Preview } = $props();

	let strip: HTMLElement | undefined = $state();
	let scrubX: number | null = $state(null);
	let thumbs = $state<Record<string, string>>({});

	$effect(() => {
		for (const r of session.rows) {
			if (thumbs[r.file] !== undefined) continue;
			thumbs[r.file] = '';
			thumbnail(photoUrl(session.folder, r.file)).then(
				(url) => (thumbs[r.file] = url),
				() => (thumbs[r.file] = 'failed')
			);
		}
	});

	// The playing frame stays in view, centred; under the pointer while scrubbing it only moves as far as needed.
	$effect(() => {
		(strip?.children[session.frame] as HTMLElement | undefined)?.scrollIntoView({
			inline: untrack(() => scrubX) === null ? 'center' : 'nearest',
			block: 'nearest'
		});
	});

	function pick() {
		if (scrubX === null || !strip) return;
		const r = strip.getBoundingClientRect();
		const t = document
			.elementFromPoint(clamp(scrubX, r.left + 2, r.right - 2), r.top + r.height / 2)
			?.closest<HTMLElement>('.thumb');
		if (t && Number(t.dataset.i) !== session.frame) session.seek(Number(t.dataset.i));
	}

	function edgeScroll() {
		if (scrubX === null || !strip) return;
		const r = strip.getBoundingClientRect();
		const edge = 56;
		const speed =
			scrubX < r.left + edge ? -(r.left + edge - scrubX) : scrubX > r.right - edge ? scrubX - (r.right - edge) : 0;
		if (speed) {
			strip.scrollLeft += speed / 3;
			pick();
		}
		requestAnimationFrame(edgeScroll);
	}

	function down(e: PointerEvent) {
		if (e.button !== 0) return;
		strip!.setPointerCapture(e.pointerId);
		scrubX = e.clientX;
		pick();
		requestAnimationFrame(edgeScroll);
	}

	function wheel(e: WheelEvent) {
		if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
		e.preventDefault();
		strip!.scrollLeft += e.deltaY;
	}
</script>

<nav
	class="filmstrip"
	class:dim={session.busy}
	class:scrubbing={scrubX !== null}
	bind:this={strip}
	onpointerdown={down}
	onpointermove={(e) => {
		if (scrubX !== null) {
			scrubX = e.clientX;
			pick();
		}
	}}
	onpointerup={() => (scrubX = null)}
	onpointercancel={() => (scrubX = null)}
	onwheel={wheel}
>
	{#each session.rows as r, i (r.file)}
		<a
			class="thumb {r.source}"
			class:current={i === session.frame}
			class:failed={thumbs[r.file] === 'failed' || session.failed.has(i)}
			data-i={i}
			href="#{r.file}"
			draggable="false"
			onclick={(e) => e.preventDefault()}
		>
			{#if thumbs[r.file] && thumbs[r.file] !== 'failed'}
				<img src={thumbs[r.file]} alt="" draggable="false" />
			{:else}
				<span class="blank" style:aspect-ratio={r.width && r.height ? r.width / r.height : 4 / 3}></span>
			{/if}
			<span class="tip">{r.file.replace(/\.[^.]+$/, '')} <b>{r.source}</b></span>
		</a>
	{/each}
</nav>
