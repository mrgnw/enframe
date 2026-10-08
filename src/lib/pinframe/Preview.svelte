<script lang="ts">
// The preview and export screen (mockups/02-preview.html): the clip loops in the browser from the
// sidecar's crop rects, the transport island sets fps and pad, the panel exports. Space plays and pauses.
import { onMount } from 'svelte';
import Clip from './Clip.svelte';
import Export from './Export.svelte';
import { FPS_PRESETS, type PadChoice } from './clip';
import { Preview } from './preview-session.svelte';
import Strip from './PreviewStrip.svelte';
import './pin.css';
import './marking.css';
import './preview.css';

let { folder }: { folder: string } = $props();

// The page re-creates this component when the folder changes (`{#key folder}`), so initial values are right.
// svelte-ignore state_referenced_locally
const session = new Preview(folder);
onMount(() => {
	void session.load();
	return () => session.dispose();
});

const back = $derived(`/pin?folder=${encodeURIComponent(folder)}`);
const n = $derived(session.rows.length);
const PADS: { v: PadChoice; title: string }[] = [
	{ v: 'none', title: 'No padding: the crop shrinks until every photo fills it' },
	{ v: 'black', title: 'Black' },
	{ v: 'white', title: 'White' },
	{ v: 'transparent', title: 'Transparent (AVIF, MOV, WebM)' }
];

$effect(() => {
	if (!session.playing) return;
	const timer = setInterval(() => session.step(1), 1000 / session.fps);
	return () => clearInterval(timer);
});

function keydown(e: KeyboardEvent) {
	// With a modifier held only export is ours; ⌘← and the like keep their browser meaning.
	if (e.metaKey || e.ctrlKey || e.altKey) {
		if (e.key === 'e' && !e.altKey) {
			e.preventDefault();
			void session.export();
		}
		return;
	}
	if ((e.target as HTMLElement).closest('input, textarea') || !n) return;
	if (e.key === ' ') {
		e.preventDefault();
		session.playing = !session.playing;
	} else if (e.key === 'ArrowRight') {
		e.preventDefault();
		session.seek(session.frame + (e.shiftKey ? 10 : 1));
	} else if (e.key === 'ArrowLeft') {
		e.preventDefault();
		session.seek(session.frame - (e.shiftKey ? 10 : 1));
	} else if (e.key === 'Home') {
		e.preventDefault();
		session.seek(0);
	} else if (e.key === 'End') {
		e.preventDefault();
		session.seek(n - 1);
	}
}
</script>

<svelte:window onkeydown={keydown} />

{#if session.stage === 'loading'}
	<main class="stage preview"><p class="muted">Loading {session.name}…</p></main>
	<nav class="filmstrip empty"></nav>
{:else if session.stage === 'failed' || session.stage === 'empty'}
	<main class="stage preview">
		<section class="drop">
			<div>
				{#if session.stage === 'empty'}
					<h1>Nothing to <b>preview</b> yet</h1>
					<p>{session.name} has no points. Click them on the first photo and track, and the clip plays here.</p>
					<a class="btn primary" href={back}>Mark {session.name}</a>
				{:else}
					<h1>Can't open <b>{session.name}</b></h1>
					<p>{session.error}</p>
					<a class="btn primary" href={session.absFolder ? back : '/pin'}>{session.absFolder ? 'Back to the marks' : 'Choose another folder'}</a>
				{/if}
			</div>
		</section>
	</main>
	<nav class="filmstrip empty"></nav>
{:else}
	<main class="stage preview">
		<div class="dock top">
			<div class="island start">
				<span class="wordmark">pin<b>frame</b></span>
				<span class="sep desk"></span>
				<span class="muted desk">{session.name} · {n} photos</span>
			</div>

			<div class="island steps mid desk">
				<a class="step done" href={back}><span class="n">1</span>Points</a>
				<a class="step done" href={back} title="Back to fixing lost photos"><span class="n">2</span>Fix</a>
				<span class="step active"><span class="n">3</span>Export</span>
			</div>

			<a class="island end phone back" href={back}>‹ Back</a>
		</div>

		<Export {session} />

		<div class="island bc transport" class:busy={session.busy} id="transport">
			<button class="btn icon" id="play" title="Play / pause (Space)" onclick={() => (session.playing = !session.playing)}>{session.playing ? '❚❚' : '▶'}</button>
			<span class="frame"><b id="frame-n">{session.frame + 1}</b> / {n}</span>
			<span class="sep"></span>
			<label for="fps-n">fps</label>
			<div class="seg" id="fps">
				{#each FPS_PRESETS as f (f)}
					<button class:on={session.fps === f} onclick={() => session.setFps(f)}>{f}</button>
				{/each}
			</div>
			<input
				id="fps-n"
				class="num"
				type="number"
				min="1"
				max="60"
				step="1"
				value={session.fps}
				aria-label="Frames per second"
				oninput={(e) => session.setFps(e.currentTarget.valueAsNumber)}
			/>
			<span class="sep"></span>
			<span class="lbl">pad</span>
			<div class="swatches" id="pad" role="group" aria-label="Padding">
				{#each PADS as { v, title } (v)}
					<button class="swatch {v}" class:on={session.pad === v} {title} aria-label={title} disabled={session.exp.status === 'running'} onclick={() => session.setPad(v)}></button>
				{/each}
				<label class="swatch custom" class:on={session.pad === 'custom'} title="Any colour">
					<input
						type="color"
						id="custom"
						value={session.custom}
						aria-label="Custom padding colour"
						disabled={session.exp.status === 'running'}
						oninput={(e) => session.setPad('custom', e.currentTarget.value)}
					/>
				</label>
			</div>
			<span class="muted size" id="size">
				{#if session.tracking}Tracking…{:else if session.busy}Drawing {session.drawn}/{n}{:else}{session.view?.clip.w}×{session.view?.clip.h}{/if}
			</span>
			<div class="bar"></div>
		</div>

		<Clip {session} />
	</main>

	<Strip {session} />
{/if}
