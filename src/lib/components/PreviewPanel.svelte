<script lang="ts">
import type { Frame, Photo } from '$lib/db';
import { getImageSrc } from '$lib/tauri';

interface Props {
	photo: Photo;
	frames: Frame[];
	selectedFrameId?: number | null;
	exportSelection?: Set<number>;
	mode?: 'single' | 'all';
	imageWidth?: number;
	imageHeight?: number;
	onselectframe?: (id: number) => void;
	ontoggleexport?: (id: number) => void;
	onmodechange?: (mode: 'single' | 'all') => void;
	onclose?: () => void;
}

let {
	photo,
	frames,
	selectedFrameId = null,
	exportSelection = new Set(),
	mode = 'all',
	imageWidth = 0,
	imageHeight = 0,
	onselectframe,
	ontoggleexport,
	onmodechange,
	onclose
}: Props = $props();

let hasExportSelection = $derived(exportSelection.size > 0);
let selectedFrame = $derived(frames.find((f) => f.id === selectedFrameId));
let displayFrames = $derived(
	mode === 'single' && selectedFrame
		? [selectedFrame]
		: hasExportSelection
			? frames.filter((f) => exportSelection.has(f.id))
			: frames
);

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

<div class="preview-panel">
	<div class="panel-header">
		<div class="mode-tabs">
			<button
				class="mode-tab"
				class:active={mode === 'single'}
				onclick={() => onmodechange?.('single')}
			>Selected</button>
			<button
				class="mode-tab"
				class:active={mode === 'all'}
				onclick={() => onmodechange?.('all')}
			>{hasExportSelection ? `Export (${exportSelection.size})` : 'All'}</button>
		</div>
		<button class="close-btn" onclick={onclose} title="Close Preview (V)">
			<svg width="14" height="14" viewBox="0 0 20 20" fill="none">
				<path d="M15 5L5 15M5 5l10 10" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
			</svg>
		</button>
	</div>

	<div class="panel-body">
		{#if mode === 'single' && !selectedFrame}
			<div class="empty-state">Select a frame to preview</div>
		{:else if displayFrames.length === 0}
			<div class="empty-state">No frames</div>
		{:else}
			<div class="preview-grid" class:single-mode={mode === 'single'}>
				{#each displayFrames as frame (frame.id)}
					{@const px = framePx(frame)}
					{@const aspect = frame.width / frame.height}
					<button
						class="preview-card"
						class:selected={selectedFrameId === frame.id}
						onclick={(e: MouseEvent) => {
							if (e.shiftKey || e.metaKey) {
								ontoggleexport?.(frame.id);
							} else {
								onselectframe?.(frame.id);
							}
						}}
					>
						<div
							class="crop-wrapper"
							style="aspect-ratio: {aspect};"
						>
							<img
								src={getImageSrc(photo.file_path)}
								alt="Frame {frameIndex(frame)}"
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
		background: rgba(0, 0, 0, 0.85);
		backdrop-filter: blur(12px);
		border-left: 1px solid rgba(255, 255, 255, 0.08);
	}

	.panel-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 8px 8px 4px;
		border-bottom: 1px solid rgba(255, 255, 255, 0.08);
		flex-shrink: 0;
	}

	.mode-tabs {
		display: flex;
		gap: 2px;
		background: rgba(255, 255, 255, 0.06);
		border-radius: 6px;
		padding: 2px;
	}

	.mode-tab {
		padding: 4px 10px;
		font-size: 11px;
		font-weight: 500;
		color: rgba(255, 255, 255, 0.5);
		background: transparent;
		border: none;
		border-radius: 4px;
		cursor: pointer;
		transition: color 0.15s, background 0.15s;
	}

	.mode-tab:hover {
		color: rgba(255, 255, 255, 0.7);
	}

	.mode-tab.active {
		color: rgba(255, 255, 255, 0.9);
		background: rgba(255, 255, 255, 0.12);
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
		height: 100%;
		color: rgba(255, 255, 255, 0.3);
		font-size: 12px;
	}

	.preview-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 6px;
	}

	.preview-grid.single-mode {
		grid-template-columns: 1fr;
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

	.preview-card.selected {
		border-color: rgba(59, 130, 246, 0.7);
		background: rgba(59, 130, 246, 0.08);
	}

	.crop-wrapper {
		position: relative;
		width: 100%;
		overflow: hidden;
		border-radius: 4px 4px 0 0;
	}

	.crop-wrapper img {
		display: block;
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
