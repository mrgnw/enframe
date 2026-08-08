<script lang="ts">
import type { FocalPoint, Frame, Photo } from '$lib/db';
import { getImageSrc } from '$lib/tauri';
import CompositionGuide from './CompositionGuide.svelte';
import DraggableFocalPoint from './DraggableFocalPoint.svelte';
import ResizableFrame from './ResizableFrame.svelte';

interface Props {
	photo: Photo;
	frames?: Frame[];
	focalPoints?: FocalPoint[];
	selectedFrameId?: number | null;
	selectedPointId?: number | null;
	exportSelection?: Set<number>;
	onchange?: () => void;
	ondeleteframe?: (frameId: number) => void;
	onselectframe?: (frameId: number | null) => void;
	onselectpoint?: (pointId: number | null) => void;
	ontoggleexport?: (frameId: number) => void;
	onimageload?: (width: number, height: number) => void;
	oncanvasclick?: (normX: number, normY: number, altKey: boolean) => void;
	compositionLabels?: Map<number, string>;
	activeGuide?: 'off' | 'thirds' | 'phi' | 'spiral' | 'triangle' | 'diagonal';
}

let {
	photo,
	frames = $bindable([]),
	focalPoints = $bindable([]),
	selectedFrameId = null,
	selectedPointId = null,
	exportSelection = new Set(),
	onchange,
	ondeleteframe,
	onselectframe,
	onselectpoint,
	ontoggleexport,
	onimageload,
	oncanvasclick,
	compositionLabels = new Map(),
	activeGuide = 'off'
}: Props = $props();

let hasExportSelection = $derived(exportSelection.size > 0);

let containerEl = $state<HTMLDivElement | null>(null);
let imgEl = $state<HTMLImageElement | null>(null);

let renderRect = $state({ x: 0, y: 0, w: 0, h: 0 });

function computeRenderRect() {
	if (!containerEl || !imgEl || !imgEl.naturalWidth || !imgEl.naturalHeight) return;

	const cw = containerEl.clientWidth;
	const ch = containerEl.clientHeight;
	const iw = imgEl.naturalWidth;
	const ih = imgEl.naturalHeight;

	const scaleX = cw / iw;
	const scaleY = ch / ih;
	const s = Math.min(scaleX, scaleY);

	const rw = iw * s;
	const rh = ih * s;
	const rx = (cw - rw) / 2;
	const ry = (ch - rh) / 2;

	renderRect = { x: rx, y: ry, w: rw, h: rh };
}

function handleImageLoad() {
	computeRenderRect();
	if (imgEl) onimageload?.(imgEl.naturalWidth, imgEl.naturalHeight);
	if (containerEl) {
		resizeObserver?.disconnect();
		resizeObserver = new ResizeObserver(() => computeRenderRect());
		resizeObserver.observe(containerEl);
	}
}

let resizeObserver: ResizeObserver | undefined;

function handleFrameClick(frameId: number, event?: PointerEvent) {
	if (event && (event.shiftKey || event.metaKey)) {
		ontoggleexport?.(frameId);
		return;
	}
	onselectframe?.(frameId);
}

function selectPoint(pointId: number) {
	onselectpoint?.(pointId);
}

function handleOverlayClick(event: PointerEvent) {
	// Only handle clicks directly on the overlay container (blank canvas area)
	if (event.target !== event.currentTarget) return;
	if (renderRect.w === 0 || renderRect.h === 0) return;

	const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
	const normX = Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width));
	const normY = Math.max(0, Math.min(1, (event.clientY - rect.top) / rect.height));

	oncanvasclick?.(normX, normY, event.altKey);
}

function isInputFocused() {
	const el = document.activeElement;
	if (!el) return false;
	const tag = el.tagName.toLowerCase();
	return tag === 'input' || tag === 'textarea' || (el as HTMLElement).isContentEditable;
}

function handleKeyDown(event: KeyboardEvent) {
	if (event.metaKey || event.ctrlKey || event.altKey || isInputFocused()) return;

	if (event.key >= '1' && event.key <= '9') {
		const index = parseInt(event.key, 10) - 1;
		if (index < frames.length) {
			event.preventDefault();
			onselectframe?.(frames[index].id);
		}
	}
}
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="canvas-wrapper" bind:this={containerEl}>
	<img
		bind:this={imgEl}
		src={getImageSrc(photo.file_path)}
		alt={photo.title || 'Photo'}
		draggable={false}
		onload={handleImageLoad}
		class="photo"
	/>

	{#if renderRect.w > 0}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="overlay-container"
			style="left:{renderRect.x}px;top:{renderRect.y}px;width:{renderRect.w}px;height:{renderRect.h}px;"
			onpointerdown={handleOverlayClick}
		>
		{#each frames as frame, i (frame.id)}
			<ResizableFrame
				{frame}
				index={i}
				imageWidth={renderRect.w}
				imageHeight={renderRect.h}
				scale={1}
				selected={selectedFrameId === frame.id}
				exportChecked={!hasExportSelection || exportSelection.has(frame.id)}
				compositionLabel={compositionLabels.get(frame.id) ?? ''}
				onSelect={(e) => handleFrameClick(frame.id, e)}
				{onchange}
			/>
			{#if selectedFrameId === frame.id && activeGuide !== 'off'}
				<div
					class="guide-container"
					style="left:{frame.x * 100}%;top:{frame.y * 100}%;width:{frame.width * 100}%;height:{frame.height * 100}%;"
				>
					<CompositionGuide
						guide={activeGuide}
						width={frame.width * renderRect.w}
						height={frame.height * renderRect.h}
					/>
				</div>
			{/if}
		{/each}

			{#each focalPoints as point, i (point.id)}
				<DraggableFocalPoint
					{point}
					index={i}
					imageWidth={renderRect.w}
					imageHeight={renderRect.h}
					scale={1}
					selected={selectedPointId === point.id}
					onSelect={() => selectPoint(point.id)}
					{onchange}
				/>
			{/each}
		</div>
	{/if}
</div>

<style>
	.canvas-wrapper {
		position: relative;
		width: 100%;
		height: 100%;
		overflow: hidden;
		background: #1a1a1a;
	}

	.photo {
		display: block;
		width: 100%;
		height: 100%;
		object-fit: contain;
		user-select: none;
		pointer-events: none;
	}

	.overlay-container {
		position: absolute;
		pointer-events: auto;
	}

	.guide-container {
		position: absolute;
		pointer-events: none;
		z-index: 15;
	}
</style>
