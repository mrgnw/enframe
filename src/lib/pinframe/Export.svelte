<script lang="ts">
// The export panel: the feature, format, PNG frames, where to save, and the run's progress and result. With
// several features, "Export all" runs each one's export in turn and lists every clip.
import { clipUrl } from './api';
import { formatBlocked, FORMATS, type Format } from './clip';
import Done from './Done.svelte';
import type { Preview } from './preview-session.svelte';

let { session }: { session: Preview } = $props();

const TITLES: Record<Format, string> = {
	avif: 'Animated AVIF. Keeps transparency, plays in a browser as an image.',
	mp4: 'H.264. No transparency.',
	mov: 'ProRes 4444 when transparent.',
	webm: 'VP9. Keeps transparency.'
};
const NOTES: Record<Format, string> = {
	avif: 'Plays in a browser as an image.',
	mp4: 'H.264 through ffmpeg. Plays everywhere.',
	mov: 'ProRes 4444 through ffmpeg, for video editors.',
	webm: 'VP9 through ffmpeg.'
};
const LABELS: Record<Format, string> = { avif: 'AVIF', mp4: 'MP4', mov: 'MOV', webm: 'WebM' };

const running = $derived(session.exporting);
const several = $derived(session.features.length > 1);
const clips = $derived(
	session.features.map((f, i) => ({ name: f.name, exp: session.exps[i] })).filter((c) => c.exp?.status === 'done')
);
const lost = $derived(session.lost);

// A long path shows its end, where the folder's name is.
let field: HTMLInputElement | undefined = $state();
$effect(() => {
	session.outBase;
	if (field && document.activeElement !== field) field.scrollLeft = field.scrollWidth;
});
</script>

<div class="island panel export" id="export">
	<h2>Export</h2>
	{#if several}
		<div class="row"><span>Feature</span>
			<div class="seg" id="feature">
				{#each session.features as f, i (f.name)}
					{@const exp = session.exps[i]}
					<button class:on={session.selected === i} disabled={running || session.tracking} onclick={() => session.select(i)}>
						{f.name}{exp?.status === 'running' ? ' …' : exp?.status === 'done' ? ' ✓' : ''}
					</button>
				{/each}
			</div>
		</div>
	{/if}
	<div class="row"><span>Format</span>
		<div class="seg" id="format">
			{#each FORMATS as f (f)}
				{@const why = formatBlocked(f, session.pad)}
				<button class:on={session.format === f} disabled={!!why || running} title={why ?? TITLES[f]} onclick={() => session.setFormat(f)}>{LABELS[f]}</button>
			{/each}
		</div>
	</div>
	<div class="note" id="format-note">{NOTES[session.format]}</div>
	<label class="check"><input type="checkbox" bind:checked={session.frames} disabled={running} onchange={() => session.resetExport()} /> Also save each aligned photo as PNG</label>
	<div class="row"><span>Save as</span></div>
	<label class="field">
		<input
			class="path"
			bind:this={field}
			bind:value={session.outBase}
			disabled={running}
			spellcheck="false"
			aria-label="Where to save the clip, without the extension"
			oninput={() => session.resetExport()}
			onblur={() => field && (field.scrollLeft = field.scrollWidth)}
		/>
		<span class="ext">.{session.format}</span>
	</label>
	<div class="note">A <code>.json</code> with every crop is written beside it.</div>
	{#if lost}
		<div class="note warn">{lost} photo{lost === 1 ? ' is' : 's are'} still lost; the clip uses the tracker's estimate for {lost === 1 ? 'it' : 'them'}.</div>
	{/if}
	{#if running}
		{#each session.features as f, i (f.name)}
			{@const exp = session.exps[i]}
			{#if exp?.status === 'running'}
				<div class="progress" id="progress"><i style:width="{Math.round(exp.progress * 100)}%"></i></div>
				<div class="note">{several ? `${f.name}: ` : ''}{exp.text}</div>
			{/if}
		{/each}
	{:else if session.exp.status === 'done'}
		<Done {session} />
	{:else}
		{#if session.exp.status === 'failed'}
			<div class="note warn" role="alert">{session.exp.error}</div>
		{/if}
		<div class="buttons">
			<button class="btn primary" id="export-btn" disabled={!session.outBase.trim() || session.tracking} onclick={() => session.export()}>
				Export <kbd class="desk">⌘E</kbd>
			</button>
			{#if several}
				<button class="btn" id="export-all" disabled={session.tracking || session.outBases.some((b) => !b.trim())} onclick={() => session.exportAll()}>
					Export all {session.features.length}
				</button>
			{/if}
		</div>
	{/if}
	{#if several && clips.length}
		<ul class="clips" id="clips">
			{#each clips as c (c.name)}
				<li>
					<b>{c.name}</b>
					<span class="where">{c.exp.out}</span>
					<a href={clipUrl(c.exp.out)} download={c.exp.out.split('/').pop()}>Clip</a>
					<a href={clipUrl(c.exp.out.replace(/\.[^./]+$/, '.json'))} class="sidecar">Sidecar</a>
				</li>
			{/each}
		</ul>
	{/if}
</div>
