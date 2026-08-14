<script lang="ts">
import { findDestination } from '$lib/destinations';
import type { Frame } from '$lib/framer/types';

interface Props {
	imageSrc: string;
	frames: Frame[];
	selectedFrameId?: number | null;
	selection?: Set<number>;
	imageWidth?: number;
	imageHeight?: number;
	layout?: 'sidebar' | 'main';
	onselectframe?: (id: number) => void;
	ontoggleselect?: (id: number) => void;
	ontogglelayout?: () => void;
	onclose?: () => void;
}

let {
	imageSrc,
	frames,
	selectedFrameId = null,
	selection = new Set(),
	imageWidth = 0,
	imageHeight = 0,
	layout = 'sidebar',
	onselectframe,
	ontoggleselect,
	ontogglelayout,
	onclose
}: Props = $props();

let hasSelection = $derived(selection.size > 0);
let selectedFrame = $derived(frames.find((f) => f.id === selectedFrameId));
let listFrames = $derived(
	(hasSelection ? frames.filter((f) => selection.has(f.id)) : frames).filter((f) => f.id !== selectedFrameId)
);

function frameName(frame: Frame) {
	return findDestination(frame.destination)?.name ?? frame.label ?? `Frame ${frameIndex(frame)}`;
}

function framePx(frame: Frame) {
	const w = Math.round(imageWidth * frame.width);
	const h = Math.round(imageHeight * frame.height);
	return { w, h };
}

function frameRatio(frame: Frame) {
	const r = frame.width / frame.height;
	if (Math.abs(r - 16 / 9) < 0.02) return '16:9';
	if (Math.abs(r - 9 / 16) < 0.02) return '9:16';
	if (Math.abs(r - 4 / 3) < 0.02) return '4:3';
	if (Math.abs(r - 3 / 4) < 0.02) return '3:4';
	if (Math.abs(r - 3 / 2) < 0.02) return '3:2';
	if (Math.abs(r - 2 / 3) < 0.02) return '2:3';
	if (Math.abs(r - 1) < 0.02) return '1:1';
	return r.toFixed(2);
}

function frameIndex(frame: Frame) {
	return frames.indexOf(frame) + 1;
}
</script>

{#snippet crop(frame: Frame)}
	<div class="crop-wrapper" style="aspect-ratio: {frame.width / frame.height};">
		<img
			src={imageSrc}
			alt="{frameName(frame)} crop"
			draggable={false}
			style="
				position: absolute;
				width: {(1 / frame.width) * 100}%;
				height: {(1 / frame.height) * 100}%;
				left: {-(frame.x / frame.width) * 100}%;
				top: {-(frame.y / frame.height) * 100}%;
			"
		/>
	</div>
{/snippet}

<div class="preview-panel" class:main={layout === 'main'}>
	<div class="panel-header">
		<span class="panel-title">{hasSelection ? `Selected (${selection.size})` : 'Preview'}</span>
		<button
			class="close-btn"
			onclick={ontogglelayout}
			title={layout === 'main' ? 'Preview in sidebar (⇧V)' : 'Preview in main window (⇧V)'}
		>
			{#if layout === 'main'}
				<svg width="14" height="14" viewBox="0 0 20 20" fill="none">
					<rect x="3" y="4" width="14" height="12" rx="1.5" stroke="currentColor" stroke-width="1.5"/>
					<path d="M13 4v12" stroke="currentColor" stroke-width="1.5"/>
				</svg>
			{:else}
				<svg width="14" height="14" viewBox="0 0 20 20" fill="none">
					<path d="M12 4h4v4M8 16H4v-4M16 4l-5 5M4 16l5-5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			{/if}
		</button>
		<button class="close-btn" onclick={onclose} title="Close Preview (V)">
			<svg width="14" height="14" viewBox="0 0 20 20" fill="none">
				<path d="M15 5L5 15M5 5l10 10" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
			</svg>
		</button>
	</div>

	<div class="panel-body">
		<div class="hero">
			{#if selectedFrame}
				{@const px = framePx(selectedFrame)}
				<div class="hero-card">
					{@render crop(selectedFrame)}
					<div class="hero-info">
						<span class="hero-name">{frameName(selectedFrame)}</span>
						<span class="hero-dims">{px.w}&times;{px.h}</span>
						<span class="frame-ratio">{frameRatio(selectedFrame)}</span>
					</div>
				</div>
			{:else}
				<div class="empty-state">Select a frame to preview its crop</div>
			{/if}
		</div>

		{#if listFrames.length > 0}
			<div class="preview-grid">
				{#each listFrames as frame (frame.id)}
					{@const px = framePx(frame)}
					<button
						class="preview-card"
						onclick={(e: MouseEvent) => {
							if (e.shiftKey || e.metaKey) {
								ontoggleselect?.(frame.id);
							} else {
								onselectframe?.(frame.id);
							}
						}}
					>
						{@render crop(frame)}
						<div class="preview-info">
							<span class="frame-num">{frameIndex(frame)}</span>
							<span class="frame-dims">{px.w}&times;{px.h}</span>
							<span class="frame-ratio">{frameRatio(frame)}</span>
						</div>
					</button>
				{/each}
			</div>
		{/if}
	</div>
</div>

<style>
	.preview-panel {
		width: 260px;
		height: 100%;
		display: flex;
		flex-direction: column;
		padding-top: 60px;
		box-sizing: border-box;
		background: rgba(0, 0, 0, 0.85);
		backdrop-filter: blur(12px);
		border-left: 1px solid rgba(255, 255, 255, 0.08);
	}

	.preview-panel.main {
		width: 100%;
		border-left: none;
		background: rgba(0, 0, 0, 0.92);
	}

	.preview-panel.main .panel-body {
		padding: 16px;
	}

	.preview-panel.main .hero-card {
		max-width: min(60vh, 640px);
		margin: 0 auto;
	}

	.preview-panel.main .preview-grid {
		grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
		gap: 10px;
		align-items: start;
	}

	.panel-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 8px 8px 4px;
		border-bottom: 1px solid rgba(255, 255, 255, 0.08);
		flex-shrink: 0;
	}

	.panel-title {
		padding-left: 6px;
		font-size: 11px;
		font-weight: 600;
		letter-spacing: 0.05em;
		text-transform: uppercase;
		color: rgba(255, 255, 255, 0.45);
	}

	.close-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: rgba(255, 255, 255, 0.4);
		cursor: pointer;
		transition: color 0.15s, background 0.15s;
	}

	.close-btn:hover {
		color: rgba(255, 255, 255, 0.8);
		background: rgba(255, 255, 255, 0.1);
	}

	.panel-body {
		flex: 1;
		overflow-y: auto;
		padding: 8px;
	}

	.empty-state {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 24px 12px;
		text-align: center;
		color: rgba(255, 255, 255, 0.3);
		font-size: 12px;
		line-height: 1.5;
	}

	.hero {
		margin-bottom: 10px;
	}

	.hero-card {
		display: flex;
		flex-direction: column;
		border: 1px solid rgba(249, 115, 22, 0.5);
		border-radius: 8px;
		background: rgba(249, 115, 22, 0.07);
		overflow: hidden;
	}

	.hero-info {
		display: flex;
		align-items: baseline;
		gap: 6px;
		padding: 6px 8px 7px;
		font-size: 11px;
		color: rgba(255, 255, 255, 0.5);
	}

	.hero-name {
		font-weight: 600;
		color: rgba(255, 255, 255, 0.92);
	}

	.hero-dims {
		flex: 1;
		font-variant-numeric: tabular-nums;
	}

	.preview-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 6px;
	}

	.preview-card {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 0;
		border: 2px solid transparent;
		border-radius: 6px;
		background: rgba(255, 255, 255, 0.04);
		cursor: pointer;
		overflow: hidden;
		transition: border-color 0.15s, background 0.15s;
	}

	.preview-card:hover {
		background: rgba(255, 255, 255, 0.08);
		border-color: rgba(255, 255, 255, 0.15);
	}

	.crop-wrapper {
		position: relative;
		width: 100%;
		overflow: hidden;
		border-radius: 4px 4px 0 0;
	}

	.crop-wrapper img {
		display: block;
		/* ponytail: tailwind preflight caps images at 100%, which squashes every CSS crop */
		max-width: none;
		user-select: none;
		pointer-events: none;
	}

	.preview-info {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 3px 6px 5px;
		font-size: 10px;
		color: rgba(255, 255, 255, 0.5);
	}

	.frame-num {
		color: rgba(255, 255, 255, 0.7);
		font-weight: 600;
	}

	.frame-dims {
		flex: 1;
	}

	.frame-ratio {
		color: rgba(255, 255, 255, 0.35);
	}
</style>
