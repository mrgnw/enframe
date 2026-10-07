<script lang="ts">
	// The only list: one thumbnail per photo, a dot for its state, a link line across tracked runs. Press
	// and drag to scrub; near either edge it scrolls on its own. A vertical wheel scrolls it sideways.
	import { photoUrl } from './api';
	import type { Session } from './session.svelte';
	import { thumbnail } from './thumbs';
	import { clamp } from './types';

	let { session }: { session: Session } = $props();

	let strip: HTMLElement | undefined = $state();
	let scrubX: number | null = $state(null);
	let thumbs = $state<Record<string, string>>({});

	$effect(() => {
		for (const p of session.photos) {
			if (thumbs[p.file] !== undefined) continue;
			thumbs[p.file] = '';
			thumbnail(photoUrl(session.folder, p.file)).then(
				(url) => (thumbs[p.file] = url),
				() => (thumbs[p.file] = 'failed')
			);
		}
	});

	// Jumps centre the current thumb smoothly; steps and scrubbing only keep it in view.
	$effect(() => {
		const { jump } = session.scroll;
		const el = strip?.children[session.current] as HTMLElement | undefined;
		el?.scrollIntoView(
			jump ? { inline: 'center', block: 'nearest', behavior: 'smooth' } : { inline: 'nearest', block: 'nearest' }
		);
	});

	function tip(p: (typeof session.photos)[number], listed: boolean) {
		const state =
			p.source === 'mark'
				? `your mark${listed ? ' · in fix list' : ''}`
				: p.source === 'skipped'
					? `skipped · feature hidden${listed ? ' · in fix list' : ''}`
					: p.source === 'lost'
						? `lost · ${p.feats} features · estimated`
						: `${p.feats} features`;
		return session.stage === 'points' ? '' : `${state} · ${p.scale.toFixed(2)}×`;
	}

	function pick() {
		if (scrubX === null || !strip) return;
		const r = strip.getBoundingClientRect();
		const t = document
			.elementFromPoint(clamp(scrubX, r.left + 2, r.right - 2), r.top + r.height / 2)
			?.closest<HTMLElement>('.thumb');
		if (t && Number(t.dataset.i) !== session.current) session.go(Number(t.dataset.i));
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
		if (e.button !== 0 || session.stage === 'points') return;
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
	class:empty={!session.photos.length}
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
	{#each session.photos as p, i (p.file)}
		{@const listed = session.fixList.has(p.file) && (p.source === 'mark' || p.source === 'skipped')}
		{@const cls = session.stage === 'points' ? (i === 0 ? 'mark' : 'pending') : p.source}
		<a
			class="thumb {cls}"
			class:listed
			class:current={i === session.current}
			class:failed={thumbs[p.file] === 'failed'}
			data-i={i}
			href="#{p.name}"
			onclick={(e) => e.preventDefault()}
		>
			{#if thumbs[p.file] && thumbs[p.file] !== 'failed'}
				<img src={thumbs[p.file]} alt="" draggable="false" />
			{:else}
				<span class="blank" style:aspect-ratio={p.w && p.h ? p.w / p.h : 4 / 3}></span>
			{/if}
			<span class="tip">{p.name} <b>{tip(p, listed)}</b></span>
		</a>
	{/each}
</nav>
