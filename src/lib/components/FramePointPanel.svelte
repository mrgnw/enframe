<script lang="ts">
import { Pin, Ratio, Trash2 } from '@lucide/svelte';
import { clipsRequiredPoints } from '$lib/composition';
import { DESTINATIONS, destinationHasRatio, findDestination } from '$lib/destinations';
import type { FocalPoint, Frame, SelectMode } from '$lib/framer/types';

const PINNED_KEY = 'enframe.framePointPanel.pinned';

function loadPinned(): boolean {
	if (typeof localStorage === 'undefined') return false;
	return localStorage.getItem(PINNED_KEY) === 'true';
}

interface Props {
	frames: Frame[];
	focalPoints: FocalPoint[];
	imageWidth: number;
	imageHeight: number;
	selectedFrameId?: number | null;
	selectedPointId?: number | null;
	selection?: Set<number>;
	compositionLabels?: Map<number, string>;
	expanded?: boolean;
	onselectframe?: (frameId: number | null, mode?: SelectMode) => void;
	onselectpoint?: (pointId: number | null) => void;
	ontoggleselect?: (frameId: number) => void;
	onselectall?: () => void;
	onclearselection?: () => void;
	ondeleteselection?: () => void;
	onclaim?: (frameId: number, geometry: Partial<Frame>) => Promise<void> | void;
	onlabel?: (frameId: number, label: string) => Promise<void> | void;
	onchange?: () => void;
}

let {
	frames,
	focalPoints,
	imageWidth,
	imageHeight,
	selectedFrameId = null,
	selectedPointId = null,
	selection = new Set(),
	compositionLabels = new Map(),
	expanded = false,
	onselectframe,
	onselectpoint,
	ontoggleselect,
	onselectall,
	onclearselection,
	ondeleteselection,
	onclaim,
	onlabel,
	onchange
}: Props = $props();

let selectionCount = $derived(selection.size);
let hasSelection = $derived(selectionCount > 0);
let menu = $state<{ x: number; y: number } | null>(null);

let panelEl = $state<HTMLDivElement | null>(null);
let editingLabelId = $state<number | null>(null);
let hideTimer: ReturnType<typeof setTimeout> | null = null;
let visible = $state(false);
let pinned = $state(loadPinned());

$effect(() => {
	if (expanded) {
		visible = true;
		if (pinned) {
			if (hideTimer) clearTimeout(hideTimer);
		} else {
			resetHideTimer();
		}
	} else {
		visible = false;
	}
});

function resetHideTimer() {
	if (hideTimer) clearTimeout(hideTimer);
	hideTimer = setTimeout(() => {
		if (expanded && !pinned) visible = false;
	}, 3000);
}

function handleInteraction() {
	visible = true;
	if (!pinned) resetHideTimer();
}

function togglePinned() {
	pinned = !pinned;
	localStorage.setItem(PINNED_KEY, String(pinned));
}

function clickFrame(frame: Frame, event: MouseEvent) {
	onselectframe?.(frame.id, event.shiftKey || event.metaKey || event.ctrlKey ? 'toggle' : 'replace');
}

// Right-click acts on what you clicked: a row outside the selection becomes the selection first.
function openMenu(frame: Frame, event: MouseEvent) {
	event.preventDefault();
	if (!selection.has(frame.id)) onselectframe?.(frame.id, 'replace');
	menu = { x: event.clientX, y: event.clientY };
	handleInteraction();
}

function runMenu(action?: () => void) {
	menu = null;
	action?.();
}

function swapOrientation(frame: Frame) {
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

function applyRatioPreset(frame: Frame, ratioW: number, ratioH: number) {
	if (frame.destination && !destinationHasRatio(frame.destination, ratioW / ratioH)) {
		frame.destination = null;
	}
	applyRatio(frame, ratioW, ratioH);
}

function applyDestination(frame: Frame, slug: string) {
	const destination = findDestination(slug);
	if (!destination) {
		frame.destination = null;
		persistFrame(frame);
		return;
	}
	frame.destination = slug;
	applyRatio(frame, destination.ratio.w, destination.ratio.h);
}

async function saveLabel(frame: Frame, value: string) {
	editingLabelId = null;
	const label = value.trim();
	if (label === (frame.label ?? '')) return;
	await onlabel?.(frame.id, label);
	onchange?.();
}

async function persistFrame(frame: Frame) {
	await onclaim?.(frame.id, {
		x: frame.x,
		y: frame.y,
		width: frame.width,
		height: frame.height,
		rotation: frame.rotation,
		destination: frame.destination ?? null
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
		<div class="panel-header">
			{#if hasSelection}
				<span class="selection-count">{selectionCount} selected</span>
				<button class="action-btn danger" title="Delete the selected frames (⌫)" onclick={() => ondeleteselection?.()}>
					<Trash2 size={11} /> Delete
				</button>
				<button class="action-btn" title="Clear the selection (Esc)" onclick={() => onclearselection?.()}>None</button>
			{:else if frames.length > 0}
				<span class="selection-count muted">Shift-click to select several</span>
			{/if}
			{#if frames.length > 0}
				<button class="action-btn" title="Select every frame (⌘A)" onclick={() => onselectall?.()}>All</button>
			{/if}
			<button
				class="pin-btn"
				class:active={pinned}
				title={pinned ? 'Unpin panel (auto-hides after 3s again)' : 'Pin panel open'}
				onclick={togglePinned}
			>
				<Pin size={12} />
			</button>
		</div>

		<div class="panel-body">
		{#if frames.length > 0}
			<div class="section-label">Frames</div>
			{#each frames as frame, i (frame.id)}
			{@const isSelected = selectedFrameId === frame.id}
			{@const portrait = isPortrait(frame)}
			{@const isAuto = frame.is_auto}
			{@const clipped = clipsRequiredPoints(frame, frame.id, focalPoints)}
			{@const inSelection = selection.has(frame.id)}
			<div
				class="panel-row"
				class:selected={isSelected || inSelection}
				class:auto-frame={isAuto}
				class:clips-required={clipped}
				class:unselected={hasSelection && !inSelection}
				onclick={(e: MouseEvent) => clickFrame(frame, e)}
				oncontextmenu={(e: MouseEvent) => openMenu(frame, e)}
				onkeydown={() => {}}
				role="button"
				tabindex="0"
			>
				<button
					class="export-check"
					class:checked={inSelection}
					class:has-selection={hasSelection}
					title={inSelection ? 'Remove from selection' : 'Add to selection'}
					onclick={(e: MouseEvent) => { e.stopPropagation(); ontoggleselect?.(frame.id); }}
				>
					{#if inSelection}
						<svg width="10" height="10" viewBox="0 0 16 16" fill="none"><path d="M3 8l3.5 3.5L13 5" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
					{/if}
				</button>
				{#if editingLabelId === frame.id}
					<input
						class="label-input"
						value={frame.label ?? ''}
						onclick={(e: MouseEvent) => e.stopPropagation()}
						onblur={(e: FocusEvent & { currentTarget: HTMLInputElement }) => saveLabel(frame, e.currentTarget.value)}
						onkeydown={(e: KeyboardEvent & { currentTarget: HTMLInputElement }) => {
							e.stopPropagation();
							if (e.key === 'Enter') e.currentTarget.blur();
							else if (e.key === 'Escape') editingLabelId = null;
						}}
						{@attach (node: HTMLInputElement) => node.select()}
					/>
				{:else if isAuto}
					<span class="row-label auto-label">{frame.label}</span>
				{:else}
					<button
						class="row-label label-btn"
						title="Rename frame"
						onclick={(e: MouseEvent) => { e.stopPropagation(); editingLabelId = frame.id; handleInteraction(); }}
					>{frame.label || i + 1}</button>
				{/if}

					<button
						class="icon-btn"
						title={portrait ? 'Swap orientation to landscape' : 'Swap orientation to portrait'}
						aria-label={portrait ? 'Swap orientation to landscape' : 'Swap orientation to portrait'}
						onclick={() => { swapOrientation(frame); handleInteraction(); }}
					>
						<Ratio size={12} />
					</button>

				<select
					class="dest-select"
					class:assigned={!!frame.destination}
					title="Export destination"
					value={frame.destination ?? ''}
					onclick={(e: MouseEvent) => e.stopPropagation()}
					onchange={(e: Event & { currentTarget: HTMLSelectElement }) => { applyDestination(frame, e.currentTarget.value); handleInteraction(); }}
				>
					<option value="">no destination</option>
					{#each DESTINATIONS as destination (destination.slug)}
						<option value={destination.slug}>{destination.name}</option>
					{/each}
				</select>

				<div class="ratio-presets">
					{#key portrait}
						{#each RATIOS as ratio (ratio.w)}
							{@const rw = portrait ? ratio.h : ratio.w}
							{@const rh = portrait ? ratio.w : ratio.h}
							<button
								class="ratio-btn"
								onclick={() => { applyRatioPreset(frame, rw, rh); handleInteraction(); }}
							>{rw}:{rh}</button>
						{/each}
					{/key}
				</div>
				{#if clipped}
					<span class="clip-warning" title="Crops out a required focal point">&#9888;</span>
				{/if}
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
	</div>

	{#if menu}
		<div
			class="menu-backdrop"
			onclick={() => (menu = null)}
			oncontextmenu={(e: MouseEvent) => { e.preventDefault(); menu = null; }}
			role="presentation"
		></div>
		<div class="context-menu" style:left="{menu.x}px" style:top="{menu.y}px" role="menu">
			<button role="menuitem" onclick={() => runMenu(ondeleteselection)}>
				<Trash2 size={12} /> Delete {selectionCount} frame{selectionCount === 1 ? '' : 's'}
			</button>
			<button role="menuitem" onclick={() => runMenu(onselectall)}>Select all frames</button>
			<button role="menuitem" onclick={() => runMenu(onclearselection)}>Select none</button>
		</div>
	{/if}
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
		/* ponytail: toolbar above the panel is ~3rem tall, plus the 1rem inset top and bottom */
		max-height: calc(100dvh - 6rem);
		min-height: 0;
		animation: panel-in 0.15s ease;
	}

	.panel-body {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-height: 0;
		overflow-y: auto;
		overscroll-behavior: contain;
	}

	@keyframes panel-in {
		from { opacity: 0; transform: translateY(-4px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.panel-header {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 4px;
	}

	.selection-count {
		margin-right: auto;
		font-size: 10px;
		font-weight: 600;
		color: #22c55e;
		padding-left: 2px;
	}

	.selection-count.muted {
		color: rgba(255, 255, 255, 0.3);
		font-weight: 500;
	}

	.action-btn {
		display: flex;
		align-items: center;
		gap: 3px;
		padding: 2px 6px;
		border: 1px solid rgba(255, 255, 255, 0.15);
		border-radius: 4px;
		background: transparent;
		color: rgba(255, 255, 255, 0.6);
		font-family: inherit;
		font-size: 10px;
		cursor: pointer;
		transition: background 0.15s, color 0.15s, border-color 0.15s;
	}

	.action-btn:hover {
		background: rgba(255, 255, 255, 0.12);
		color: white;
		border-color: rgba(255, 255, 255, 0.3);
	}

	.action-btn.danger {
		color: #f87171;
		border-color: rgba(248, 113, 113, 0.35);
	}

	.action-btn.danger:hover {
		background: rgba(239, 68, 68, 0.2);
		color: white;
	}

	.menu-backdrop {
		position: fixed;
		inset: 0;
		z-index: 60;
	}

	.context-menu {
		position: fixed;
		z-index: 61;
		display: flex;
		flex-direction: column;
		min-width: 160px;
		padding: 4px;
		background: rgba(20, 20, 20, 0.95);
		backdrop-filter: blur(12px);
		border: 1px solid rgba(255, 255, 255, 0.12);
		border-radius: 10px;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
		animation: panel-in 0.1s ease;
	}

	.context-menu button {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 8px;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: rgba(255, 255, 255, 0.8);
		font-family: inherit;
		font-size: 12px;
		text-align: left;
		cursor: pointer;
	}

	.context-menu button:hover {
		background: rgba(255, 255, 255, 0.12);
		color: white;
	}

	.pin-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: rgba(255, 255, 255, 0.35);
		cursor: pointer;
		transition: background 0.15s, color 0.15s;
	}

	.pin-btn:hover {
		background: rgba(255, 255, 255, 0.1);
		color: white;
	}

	.pin-btn.active {
		color: #f59e0b;
	}

	.section-label {
		font-size: 10px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: rgba(255, 255, 255, 0.4);
		padding: 8px 4px 4px;
		position: sticky;
		top: -4px;
		z-index: 1;
		background: rgba(10, 10, 10, 0.92);
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

	.export-check {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 14px;
		height: 14px;
		border: 1.5px solid rgba(255, 255, 255, 0.25);
		border-radius: 3px;
		background: transparent;
		color: transparent;
		cursor: pointer;
		padding: 0;
		flex-shrink: 0;
		transition: border-color 0.15s, background 0.15s, color 0.15s;
	}

	.export-check:hover {
		border-color: rgba(255, 255, 255, 0.5);
	}

	.export-check.checked {
		border-color: #22c55e;
		background: #22c55e;
		color: white;
	}

	.panel-row.unselected {
		opacity: 0.45;
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

	.panel-row.clips-required {
		border-color: rgba(239, 68, 68, 0.55);
		background: rgba(239, 68, 68, 0.08);
	}

	.clip-warning {
		font-size: 11px;
		color: #f97316;
		margin-left: 2px;
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

	.label-btn {
		border: none;
		background: transparent;
		padding: 0;
		text-align: left;
		cursor: text;
		font-family: inherit;
	}

	.label-btn:hover {
		color: white;
	}

	.label-input {
		width: 96px;
		padding: 1px 4px;
		border: 1px solid rgba(245, 158, 11, 0.6);
		border-radius: 4px;
		background: rgba(0, 0, 0, 0.4);
		color: white;
		font-family: inherit;
		font-size: 12px;
		font-weight: 600;
	}

	.label-input:focus {
		outline: none;
		border-color: #f59e0b;
	}

	.dest-select {
		margin-left: auto;
		padding: 2px 4px;
		border: 1px solid rgba(255, 255, 255, 0.15);
		border-radius: 4px;
		background: transparent;
		color: rgba(255, 255, 255, 0.4);
		font-family: inherit;
		font-size: 10px;
		cursor: pointer;
		transition: color 0.15s, border-color 0.15s;
	}

	.dest-select:hover {
		color: white;
		border-color: rgba(255, 255, 255, 0.3);
	}

	.dest-select.assigned {
		color: #f59e0b;
		border-color: rgba(245, 158, 11, 0.4);
	}

	.dest-select option {
		background: #1a1a1a;
		color: white;
	}

	.ratio-presets {
		display: flex;
		gap: 2px;
		margin-left: 4px;
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
