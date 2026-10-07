<script lang="ts">
	// The export panel: format, PNG frames, where to save, and the run's progress and result.
	import { formatBlocked, FORMATS, type Format } from './clip';
	import Done from './Done.svelte';
	import type { Preview } from './session.svelte';

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

	const running = $derived(session.exp.status === 'running');
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
	{#if session.exp.status === 'done'}
		<Done {session} />
	{:else if running}
		<div class="progress" id="progress"><i style:width="{Math.round(session.exp.progress * 100)}%"></i></div>
		<div class="note">{session.exp.text}</div>
	{:else}
		{#if session.exp.status === 'failed'}
			<div class="note warn" role="alert">{session.exp.error}</div>
		{/if}
		<button class="btn primary" id="export-btn" disabled={!session.outBase.trim() || session.tracking} onclick={() => session.export()}>
			Export <kbd class="desk">⌘E</kbd>
		</button>
	{/if}
</div>
