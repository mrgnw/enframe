<script lang="ts">
import { RotateCw } from '@lucide/svelte';
import { isAutoFrame } from '$lib/composition';
import type { FocalPoint, Frame } from '$lib/db';
import { updateFrame } from '$lib/db';

interface Props {
	frames: Frame[];
	focalPoints: FocalPoint[];
	imageWidth: number;
	imageHeight: number;
	selectedFrameId?: number | null;
	selectedPointId?: number | null;
	compositionLabels?: Map<number, string>;
	expanded?: boolean;
	onselectframe?: (frameId: number | null) => void;
	onselectpoint?: (pointId: number | null) => void;
	onchange?: () => void;
}

let {
	frames,
	focalPoints,
	imageWidth,
	imageHeight,
	selectedFrameId = null,
	selectedPointId = null,
	compositionLabels = new Map(),
	expanded = false,
	onselectframe,
	onselectpoint,
	onchange
}: Props = $props();

let panelEl = $state<HTMLDivElement | null>(null);
let hideTimer: ReturnType<typeof setTimeout> | null = null;
let visible = $state(false);

$effect(() => {
	if (expanded) {
		visible = true;
		resetHideTimer();
	} else {
		visible = false;
	}
});

function resetHideTimer() {
	if (hideTimer) clearTimeout(hideTimer);
	hideTimer = setTimeout(() => {
		if (expanded) visible = false;
	}, 3000);
}

function handleInteraction() {
	visible = true;
	resetHideTimer();
}

function rotateFrame(frame: Frame) {
	const imgW = imageWidth || 1;
	const imgH = imageHeight || 1;

	const pxW = frame.width * imgW;
	const pxH = frame.height * imgH;

	// Swap pixel dimensions
	let newNormW = pxH / imgW;
	let newNormH = pxW / imgH;

	// Scale down proportionally if it overflows the image
	const scale = Math.min(1, 1 / newNormW, 1 / newNormH);
	newNormW *= scale;
	newNormH *= scale;

	const cx = frame.x + frame.width / 2;
	const cy = frame.y + frame.height / 2;
	frame.width = newNormW;
	frame.height = newNormH;
	frame.x = Math.max(0, Math.min(1 - frame.width, cx - frame.width / 2));
	frame.y = Math.max(0, Math.min(1 - frame.height, cy - frame.height / 2));
	persistFrame(frame);
}

function applyRatio(frame: Frame, ratioW: number, ratioH: number) {
	const imgW = imageWidth || 1;
	const imgH = imageHeight || 1;
	const targetRatio = ratioW / ratioH;

	// Convert normalized frame to pixel dimensions
	const currentPxW = frame.width * imgW;
	const currentPxH = frame.height * imgH;

	// Preserve area in pixels, apply target ratio
	const currentArea = currentPxW * currentPxH;
	const newPxW = Math.sqrt(currentArea * targetRatio);
	const newPxH = newPxW / targetRatio;

	// Convert back to normalized coordinates
	const cx = frame.x + frame.width / 2;
	const cy = frame.y + frame.height / 2;
	frame.width = Math.max(0.05, Math.min(1, newPxW / imgW));
	frame.height = Math.max(0.05, Math.min(1, newPxH / imgH));
	frame.x = Math.max(0, Math.min(1 - frame.width, cx - frame.width / 2));
	frame.y = Math.max(0, Math.min(1 - frame.height, cy - frame.height / 2));
	persistFrame(frame);
}

async function persistFrame(frame: Frame) {
	await updateFrame(frame.id, {
		x: frame.x,
		y: frame.y,
		width: frame.width,
		height: frame.height,
		rotation: frame.rotation
	});
	onchange?.();
}

const RATIOS = [
	{ w: 16, h: 9 },
	{ w: 4, h: 3 },
	{ w: 3, h: 2 },
	{ w: 1, h: 1 }
];

function isPortrait(frame: Frame): boolean {
	const pxW = frame.width * (imageWidth || 1);
	const pxH = frame.height * (imageHeight || 1);
	return pxH > pxW;
}
</script>

{#if expanded && visible}
	<div
		class="frame-point-panel"
		bind:this={panelEl}
		onmouseenter={handleInteraction}
		onmousemove={handleInteraction}
		onfocusin={handleInteraction}
		role="region"
		aria-label="Frames and points"
	>
		{#if frames.length > 0}
			<div class="section-label">Frames</div>
			{#each frames as frame, i (frame.id)}
			{@const isSelected = selectedFrameId === frame.id}
			{@const portrait = isPortrait(frame)}
			{@const isAuto = isAutoFrame(frame.label)}
			<div
				class="panel-row"
				class:selected={isSelected}
				class:auto-frame={isAuto}
				onclick={() => onselectframe?.(frame.id)}
				onkeydown={() => {}}
				role="button"
				tabindex="0"
			>
				<span class="row-label" class:auto-label={isAuto}>{isAuto ? frame.label : i + 1}</span>

					<button class="icon-btn" title="Rotate frame (swap W/H)" onclick={() => { rotateFrame(frame); handleInteraction(); }}>
						<RotateCw size={12} />
					</button>

				<div class="ratio-presets">
					{#key portrait}
						{#each RATIOS as ratio (ratio.w)}
							{@const rw = portrait ? ratio.h : ratio.w}
							{@const rh = portrait ? ratio.w : ratio.h}
							<button
								class="ratio-btn"
								onclick={() => { applyRatio(frame, rw, rh); handleInteraction(); }}
							>{rw}:{rh}</button>
						{/each}
					{/key}
				</div>
				{#if compositionLabels.has(frame.id)}
					<span class="comp-label">{compositionLabels.get(frame.id)}</span>
				{/if}
			</div>
			{/each}
		{/if}

		{#if focalPoints.length > 0}
			<div class="section-label">Points</div>
			{#each focalPoints as point, i (point.id)}
				{@const isSelected = selectedPointId === point.id}
				<div
					class="panel-row point-row"
					class:selected={isSelected}
					onclick={() => onselectpoint?.(point.id)}
					onkeydown={() => {}}
					role="button"
					tabindex="0"
				>
					<span class="row-label point-label">•{i + 1}</span>
				</div>
			{/each}
		{/if}
	</div>
{/if}

<style>
	.frame-point-panel {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 8px;
		background: rgba(0, 0, 0, 0.7);
		backdrop-filter: blur(12px);
		border-radius: 12px;
		border: 1px solid rgba(255, 255, 255, 0.1);
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
		max-height: 300px;
		overflow-y: auto;
		animation: panel-in 0.15s ease;
	}

	@keyframes panel-in {
		from { opacity: 0; transform: translateY(-4px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.section-label {
		font-size: 10px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: rgba(255, 255, 255, 0.4);
		padding: 2px 4px;
	}

	.panel-row {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 4px 6px;
		border-radius: 8px;
		border: 1px solid transparent;
		cursor: pointer;
		transition: border-color 0.15s, background 0.15s;
	}

	.panel-row:hover {
		background: rgba(255, 255, 255, 0.05);
	}

	.panel-row.selected {
		border-color: #f59e0b;
		background: rgba(245, 158, 11, 0.1);
	}

	.point-row.selected {
		border-color: #ef4444;
		background: rgba(239, 68, 68, 0.1);
	}

	.row-label {
		font-size: 12px;
		font-weight: 600;
		color: rgba(255, 255, 255, 0.7);
		min-width: 20px;
	}

	.point-label {
		color: #ef4444;
	}

	.auto-label {
		color: #22d3ee;
		font-size: 10px;
		letter-spacing: 0.02em;
	}

	.auto-frame {
		border-color: rgba(34, 211, 238, 0.2);
	}

	.comp-label {
		font-size: 9px;
		color: rgba(255, 255, 255, 0.35);
		white-space: nowrap;
		margin-left: 4px;
	}

	.icon-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: rgba(255, 255, 255, 0.5);
		cursor: pointer;
		transition: background 0.15s, color 0.15s;
	}

	.icon-btn:hover {
		background: rgba(255, 255, 255, 0.15);
		color: white;
	}

	.ratio-presets {
		display: flex;
		gap: 2px;
		margin-left: auto;
		perspective: 200px;
	}

	.ratio-btn {
		padding: 2px 6px;
		border: 1px solid rgba(255, 255, 255, 0.15);
		border-radius: 4px;
		background: transparent;
		color: rgba(255, 255, 255, 0.5);
		font-size: 10px;
		cursor: pointer;
		transition: background 0.15s, color 0.15s, border-color 0.15s;
		animation: flip-in 0.3s ease;
	}

	@keyframes flip-in {
		0% { transform: rotateX(90deg); opacity: 0.3; }
		100% { transform: rotateX(0deg); opacity: 1; }
	}

	.ratio-btn:hover {
		background: rgba(255, 255, 255, 0.15);
		color: white;
		border-color: rgba(255, 255, 255, 0.3);
	}
</style>
