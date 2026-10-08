<script lang="ts">
// The photo with the overlay: trail and points, the ghost at the cursor on a lost photo, the tag.
// Clicks: on a lost photo, place the ghost's points there; on a marked photo, add or remove a point.
import type { Snippet } from 'svelte';
import { photoUrl } from './api';
import type { Session } from './session.svelte';
import { pointsSvg, trailSvg } from './trail';
import { HIT_PX, inBounds, type Point } from './types';

let { session, children }: { session: Session; children?: Snippet } = $props();

let photoEl: HTMLDivElement | undefined = $state();
let W = $state(0);
let H = $state(0);
let cursor = $state<Point | null>(null);

const p = $derived(session.photo);
const editable = $derived(
	session.stage === 'points' || p.source === 'lost' || p.source === 'mark' || p.source === 'soft'
);
const ratio = $derived(p.w && p.h ? p.w / p.h : 4 / 3);
const overlay = $derived(
	session.stage === 'fix'
		? trailSvg(session.photos, session.current, W, H, session.trailN, session.trailMode, session.just)
		: pointsSvg(p.pts, session.just ? 'mark just' : 'mark', W, H)
);
/** The ghost's points at the cursor, in `p`'s 0–1 units: where a click would place the mark. */
const ghostAt = $derived.by(() => {
	const at = cursor;
	return at ? session.ghost.map(([dx, dy]) => [at[0] + dx, at[1] + dy] as Point) : null;
});
/** The whole pattern must land inside the photo: a click here is refused, the ghost shows why. */
const blocked = $derived(p.source === 'lost' && !!ghostAt && !inBounds(ghostAt));
const ghost = $derived(pointsSvg(session.ghost, `ghost${blocked ? ' blocked' : ''}`, W, H));
const tag = $derived(
	p.source === 'lost'
		? 'estimated'
		: p.source === 'skipped'
			? 'skipped · estimated'
			: p.source === 'mark'
				? `your mark · ${p.scale.toFixed(2)}×`
				: p.source === 'soft'
					? `soft mark · ${p.scale.toFixed(2)}×`
					: `${p.feats} features · ${p.scale.toFixed(2)}×`
);

$effect(() => {
	const el = photoEl;
	if (!el) return;
	const observer = new ResizeObserver(() => {
		W = el.clientWidth;
		H = el.clientHeight;
	});
	observer.observe(el);
	return () => observer.disconnect();
});

function norm(e: MouseEvent): Point {
	const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
	return [(e.clientX - r.left) / r.width, (e.clientY - r.top) / r.height];
}

function click(e: MouseEvent) {
	const n = norm(e);
	if (session.stage === 'points') {
		const hit = p.pts.findIndex(([x, y]) => Math.hypot((x - n[0]) * W, (y - n[1]) * H) <= HIT_PX);
		session.editStartPoints(hit < 0 ? [...p.pts, n] : p.pts.toSpliced(hit, 1));
		return;
	}
	if (p.source === 'tracked' || p.source === 'skipped') return;
	if (p.source === 'lost') {
		session.mark(
			session.ghost.map(([dx, dy]) => [n[0] + dx, n[1] + dy]),
			e.shiftKey
		);
		return;
	}
	const hit = p.pts.findIndex(([x, y]) => Math.hypot((x - n[0]) * W, (y - n[1]) * H) <= HIT_PX);
	if (hit < 0) session.editPoints([...p.pts, n], 'Added a point on');
	else if (p.pts.length > 1) session.editPoints(p.pts.toSpliced(hit, 1), 'Removed a point on');
}

function wheel(e: WheelEvent) {
	if (p.source !== 'lost') return;
	e.preventDefault();
	session.stepZoom(e.deltaY < 0 ? 1 : -1);
}

function loaded(e: Event) {
	const img = e.currentTarget as HTMLImageElement;
	session.setSize(p.file, img.naturalWidth, img.naturalHeight);
}
</script>

<div class="view" class:edit={editable}>
	<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
	<div
		class="photo"
		class:blocked
		bind:this={photoEl}
		style:--r={ratio}
		onclick={click}
		onwheel={wheel}
		onpointermove={(e) => (cursor = norm(e))}
		onpointerleave={() => (cursor = null)}
	>
		<img src={photoUrl(session.folder, p.file)} alt="" draggable="false" onload={loaded} />
		<svg class="trail" viewBox="0 0 {W} {H}">{@html overlay}</svg>
		{#if p.source === 'lost' && cursor}
			<svg class="trail ghost" viewBox="0 0 {W} {H}">
				<g transform="translate({cursor[0] * W} {cursor[1] * H})">{@html ghost}</g>
			</svg>
		{/if}
		{#if session.stage !== 'points' && p.pts.length}
			<span class="ptag {p.source}" style:left={session.tagAt.left} style:top={session.tagAt.top}>{tag}</span>
		{/if}
	</div>
	{@render children?.()}
</div>
