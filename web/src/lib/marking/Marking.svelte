<script lang="ts">
	// The marking screen: one photo at a time, the filmstrip below, controls floating over the photo
	// (mockups/01-marking.html). Stage `points` is the first photo's reference points (00-start.html's
	// "loaded" and "points" states); `fix` is the marking loop.
	import { onMount } from 'svelte';
	import Filmstrip from './Filmstrip.svelte';
	import { Session } from './session.svelte';
	import Stage from './Stage.svelte';
	import { patchLayout } from './trail';
	import { PATCH_PX, type TrailMode } from './types';
	import { photoUrl } from './api';

	let { folder, trailMode = 'steps' }: { folder: string; trailMode?: TrailMode } = $props();

	// The page re-creates this component when the folder changes (`{#key folder}`), so initial values are right.
	// svelte-ignore state_referenced_locally
	const session = new Session(folder, trailMode);
	onMount(() => void session.load());

	let phone = $state(false);
	let patchEl: HTMLDivElement | undefined = $state();

	const p = $derived(session.photo);
	const lost = $derived(p?.source === 'lost');
	const skipped = $derived(p?.source === 'skipped');
	const editable = $derived(lost || p?.source === 'mark');
	const folderName = $derived(folder.replace(/\/+$/, '').split('/').pop() ?? folder);
	const patch = $derived(
		session.stage === 'fix' && (editable || skipped)
			? patchLayout(session.ref, patchEl?.offsetWidth || 120, patchEl?.offsetHeight || 105, PATCH_PX)
			: null
	);
	const hint = $derived(
		session.stage === 'points'
			? p?.pts.length
				? 'Click to add a point · click a point to remove it'
				: 'Click points on the feature to hold still'
			: lost
				? 'Click the cloud to place its points · scroll to change zoom'
				: skipped
					? 'Skipped: the feature is hidden here, so the clip uses the estimate'
					: 'Click to add a point · click a point to remove it'
	);

	$effect(() => {
		const mq = matchMedia('(max-width: 700px)');
		phone = mq.matches;
		const onChange = () => (phone = mq.matches);
		mq.addEventListener('change', onChange);
		return () => mq.removeEventListener('change', onChange);
	});

	function keydown(e: KeyboardEvent) {
		if ((e.target as HTMLElement).closest('input, textarea')) return;
		if (session.stage === 'points') {
			if (e.key === 'Enter') void session.startTracking();
			return;
		}
		if (session.stage !== 'fix') return;
		const step = e.shiftKey ? 10 : 1;
		if (e.key === 'z' && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			session.undo();
		} else if (e.key === 'ArrowRight') {
			e.preventDefault();
			session.go(session.current + step);
		} else if (e.key === 'ArrowLeft') {
			e.preventDefault();
			session.go(session.current - step);
		} else if (e.key === 'Home') {
			e.preventDefault();
			session.go(0, true);
		} else if (e.key === 'End') {
			e.preventDefault();
			session.go(session.photos.length - 1, true);
		} else if (e.key === 'Tab' || e.key === 'l') {
			e.preventDefault();
			session.nextInList(e.shiftKey ? -1 : 1);
		} else if (e.key === 'Enter' && lost) session.mark(p.pts);
		else if (e.key === 'Backspace' || e.key === 'Delete') session.clearMark();
		else if (e.key === 's') session.skip();
		else if (e.key === 't') session.toggleTrail();
		else if (e.key === ']') session.stepZoom(1);
		else if (e.key === '[') session.stepZoom(-1);
		else if (e.key === '1') session.setZoom(1);
	}
</script>

<svelte:window onkeydown={keydown} />

{#if session.stage === 'loading'}
	<main class="stage"><p class="muted">Loading {folderName}…</p></main>
	<nav class="filmstrip empty"></nav>
{:else if session.stage === 'failed'}
	<main class="stage">
		<section class="drop">
			<div>
				<h1>Can't open <b>{folderName}</b></h1>
				<p>{session.error}</p>
				<a class="btn primary" href="/">Choose another folder</a>
			</div>
		</section>
	</main>
	<nav class="filmstrip empty"></nav>
{:else}
	<main class="stage" class:marking={session.stage === 'points'}>
		<div class="dock top">
			<div class="island start">
				<span class="wordmark desk">pin<b>frame</b></span>
				<span class="sep desk"></span>
				{#if session.stage === 'fix'}
					<span class="muted where"><b class="desk">{p.name} ·</b> {session.current + 1} / {session.photos.length}</span>
				{:else}
					<span class="muted desk">{folderName} · {session.photos.length} photos</span>
				{/if}
			</div>
			<div class="island steps mid desk">
				{#if session.stage === 'fix'}
					<span class="step done"><span class="n">1</span>Points</span>
					<span class="step active"><span class="n">2</span>Fix</span>
				{:else}
					<span class="step active"><span class="n">1</span>Points</span>
					<span class="step"><span class="n">2</span>Fix</span>
				{/if}
				<span class="step"><span class="n">3</span>Export</span>
			</div>
			{#if session.stage === 'fix'}
				<div class="island end" class:busy={session.busy}>
					<span class="count" class:ok={!session.lost}>
						<span class="dot"></span>
						<span>
							{#if session.lost}{session.lost}{phone ? '' : ' to fix'}{:else if phone}0{:else}All {session.photos.length} linked{/if}
						</span>
					</span>
					<button class="btn icon" title="Next photo in the fix list (Tab)" disabled={!session.fixList.size} onclick={() => session.nextInList()}>→</button>
					{#if session.fixed}
						<button class="btn clear-fixed" title="Remove the photos you have marked from the fix list" onclick={() => session.clearFixed()}>
							Clear <span class="desk">{session.fixed} </span>fixed
						</button>
					{/if}
					<span class="sep"></span>
					<a class="btn primary" href="/preview?folder={encodeURIComponent(folder)}">Preview</a>
					<div class="bar"></div>
				</div>
			{:else}
				<div class="island end" class:busy={session.stage === 'tracking'}>
					<span class="muted">{p.pts.length} point{p.pts.length === 1 ? '' : 's'}</span>
					<span class="sep"></span>
					<button class="btn primary" disabled={!p.pts.length || session.stage === 'tracking'} onclick={() => session.startTracking()}>
						{session.stage === 'tracking' ? 'Tracking…' : 'Track'} <kbd class="desk">⏎</kbd>
					</button>
					<div class="bar"></div>
				</div>
			{/if}
		</div>

		<Stage {session}>
			{#if session.toast}
				<div class="overlay-msg island toast">
					<span>{session.toast.text}</span>
					{#if session.toast.canUndo}
						<button class="btn" onclick={() => session.undo()}>Undo <kbd class="desk">⌘Z</kbd></button>
					{/if}
				</div>
			{:else if session.stage === 'points' || editable || skipped}
				<div class="overlay-msg hint desk">{hint}</div>
			{/if}
		</Stage>

		{#if session.stage === 'fix'}
			<div class="dock bottom">
				{#if editable || skipped}
					<div class="island looking start">
						<div
							class="patch"
							bind:this={patchEl}
							style:background-image="url({photoUrl(folder, session.ref.file)})"
							style:background-size={patch?.size}
							style:background-position={patch?.position}
						>
							<svg class="trail" viewBox="0 0 {patchEl?.offsetWidth || 120} {patchEl?.offsetHeight || 105}">{@html patch?.points ?? ''}</svg>
						</div>
						<div class="label">
							{#if lost}
								Looking for<b>{session.ref.name} · {session.ref.scale.toFixed(2)}×</b>
								<span class="more">
									{#if phone}tap where it is now{:else}click where it is now<br />or <kbd>⏎</kbd> to accept the estimate{/if}
								</span>
								<button class="btn" title="The feature is hidden in this photo (a building, trees). The clip keeps it at the estimate." onclick={() => session.skip()}>
									Skip, hidden here <kbd class="desk">S</kbd>
								</button>
							{:else if skipped}
								Skipped<b>feature hidden</b>
								<button class="btn" onclick={() => session.clearMark()}>Unskip <kbd class="desk">⌫</kbd></button>
							{:else}
								Your mark<b>{p.pts.length} points · {p.scale.toFixed(2)}×</b>
								{#if session.current > 0}
									<button class="btn" onclick={() => session.clearMark()}>Clear mark <kbd class="desk">⌫</kbd></button>
								{/if}
							{/if}
						</div>
					</div>
				{/if}
				<div class="island nav mid phone">
					<button class="btn icon" title="Previous photo" onclick={() => session.go(session.current - 1)}>‹</button>
					<button class="btn icon" title="Next photo" onclick={() => session.go(session.current + 1)}>›</button>
				</div>
				<div class="island tools end">
					<span class="legend" title="Path of the points' centre through neighbouring photos: dashed = estimated, ring = your mark"><i class="b"></i><span class="desk">before</span><i class="a"></i><span class="desk">after</span></span>
					<button class="btn icon" title="Fewer neighbours" onclick={() => session.trailLess()}>−</button>
					<span class="val" class:off={!session.trailN} title="Neighbours shown each side (T toggles)">{session.trailN || 'off'}</span>
					<button class="btn icon" title="More neighbours" onclick={() => session.trailMore()}>+</button>
					{#if lost}
						<span class="sep"></span>
						<span class="muted desk">zoom</span>
						<button class="btn icon" title="Zoom out ([)" onclick={() => session.stepZoom(-1)}>−</button>
						<span class="val">{+session.zoom.toFixed(2)}×</span>
						<button class="btn icon" title="Zoom in (])" onclick={() => session.stepZoom(1)}>+</button>
					{/if}
				</div>
			</div>
		{/if}
	</main>

	<Filmstrip {session} />
{/if}
