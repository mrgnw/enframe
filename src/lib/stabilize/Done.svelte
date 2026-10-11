<script lang="ts">
// The export's done state: the clip itself where the browser can play it, and the ways to get the file.
import { clipUrl, revealClip } from './api';
import type { Preview } from './preview-session.svelte';

let { session }: { session: Preview } = $props();

const out = $derived(session.exp.out);
const url = $derived(clipUrl(out));
const name = $derived(out.split('/').pop() ?? out);
const ext = $derived(out.slice(out.lastIndexOf('.') + 1).toLowerCase());
// MP4 and WebM play in a browser. MOV (ProRes 4444) no browser plays but Safari's.
const playable = $derived(ext === 'mp4' || ext === 'webm');
// A browser that cannot decode the clip says so with an error; the download link is left.
let broken = $state(false);
let revealError = $state('');
const clip = $derived(session.view?.clip);

async function reveal() {
	revealError = '';
	try {
		await revealClip(out);
	} catch (e) {
		revealError = (e as Error).message;
	}
}
</script>

<div class="done" id="done">
	<b>Wrote {name}</b>
	{#if playable && !broken && clip}
		<div class="shown" class:checker={session.pad === 'transparent'} style:--r="{clip.w} / {clip.h}">
			<video id="clip-shown" src={url} width={clip.w} height={clip.h} controls loop muted playsinline autoplay onerror={() => (broken = true)}></video>
		</div>
	{:else if !playable}
		<span class="note">A browser cannot play {ext.toUpperCase()}; open it in a video editor.</span>
	{:else if broken}
		<span class="note">This browser cannot play the clip; download it instead.</span>
	{/if}
	<span class="meta">{session.exp.clip} · {session.rows.length} frames · {session.fps} fps · {session.exp.seconds.toFixed(1)} s</span>
	<span class="where">{out}</span>
	<span class="links">
		<a href={url} download={name} id="download">Download</a>
		<a href={clipUrl(out.replace(/\.[^./]+$/, '.json'))} download={name.replace(/\.[^.]+$/, '.json')} id="sidecar">Sidecar .json</a>
		{#if session.exp.reveal}
			<button class="link" id="reveal" onclick={reveal}>Reveal in Finder</button>
		{/if}
	</span>
	{#if revealError}
		<span class="note warn" role="alert">{revealError}</span>
	{/if}
	<button class="again" onclick={() => (session.exp.status = 'idle')}>Export again</button>
</div>
