<script lang="ts">
import { clipsRequiredPoints } from '$lib/composition';
import { cycleFrameId, type FocalPoint, type Frame, framesAtPoint } from '$lib/framer/types';
import CompositionGuide from './CompositionGuide.svelte';
import DraggableFocalPoint from './DraggableFocalPoint.svelte';
import ResizableFrame from './ResizableFrame.svelte';

interface Props {
	imageSrc: string;
	imageName?: string;
	frames?: Frame[];
	focalPoints?: FocalPoint[];
	selectedFrameId?: number | null;
	selectedPointId?: number | null;
	selection?: Set<number>;
	onchange?: () => void;
	onclaimframe?: (
		frameId: number,
		geometry: { x: number; y: number; width?: number; height?: number }
	) => Promise<void> | void;
	onmovepoint?: (pointId: number, position: { x: number; y: number }) => Promise<void> | void;
	ondeleteframe?: (frameId: number) => void;
	onselectframe?: (frameId: number | null) => void;
	onselectpoint?: (pointId: number | null) => void;
	ontoggleselect?: (frameId: number) => void;
	onimageload?: (width: number, height: number) => void;
	oncanvasclick?: (normX: number, normY: number, altKey: boolean) => void;
	compositionLabels?: Map<number, string>;
	activeGuide?: 'off' | 'thirds' | 'phi' | 'spiral' | 'triangle' | 'diagonal';
}

let {
	imageSrc,
	imageName = '',
	frames = $bindable([]),
	focalPoints = $bindable([]),
	selectedFrameId = null,
	selectedPointId = null,
	selection = new Set(),
	onchange,
	onclaimframe,
	onmovepoint,
	ondeleteframe,
	onselectframe,
	onselectpoint,
	ontoggleselect,
	onimageload,
	oncanvasclick,
	compositionLabels = new Map(),
	activeGuide = 'off'
}: Props = $props();

const ZOOM_MIN = 1;
const ZOOM_MAX = 8;
const DRAG_THRESHOLD = 3;

let hasSelection = $derived(selection.size > 0);

let containerSize = $state({ w: 0, h: 0 });
let naturalSize = $state({ w: 0, h: 0 });
let fitMode = $state<'fit' | 'fill'>('fit');
let zoom = $state(1);
let pan = $state({ x: 0, y: 0 });
let panning = $state(false);

let dragOrigin: { x: number; y: number; panX: number; panY: number } | null = null;
let dragMoved = false;

function clamp(value: number, min: number, max: number) {
	return Math.min(max, Math.max(min, value));
}

let renderRect = $derived.by(() => {
	const { w: cw, h: ch } = containerSize;
	const { w: iw, h: ih } = naturalSize;
	if (cw === 0 || ch === 0 || iw === 0 || ih === 0) return { x: 0, y: 0, w: 0, h: 0 };

	const cover = Math.max(cw / iw, ch / ih);
	const contain = Math.min(cw / iw, ch / ih);
	const scale = (fitMode === 'fill' ? cover : contain) * zoom;

	const w = iw * scale;
	const h = ih * scale;

	return { x: (cw - w) / 2 + pan.x, y: (ch - h) / 2 + pan.y, w, h };
});

let pannable = $derived(renderRect.w > containerSize.w + 1 || renderRect.h > containerSize.h + 1);

function clampPan(x: number, y: number) {
	const limitX = Math.max(0, (renderRect.w - containerSize.w) / 2);
	const limitY = Math.max(0, (renderRect.h - containerSize.h) / 2);
	return { x: clamp(x, -limitX, limitX), y: clamp(y, -limitY, limitY) };
}

function observeContainer(node: HTMLElement) {
	const observer = new ResizeObserver(() => {
		containerSize = { w: node.clientWidth, h: node.clientHeight };
		pan = clampPan(pan.x, pan.y);
	});
	observer.observe(node);
	return () => observer.disconnect();
}

function handleImageLoad(event: Event) {
	const img = event.currentTarget as HTMLImageElement;
	naturalSize = { w: img.naturalWidth, h: img.naturalHeight };
	zoom = 1;
	pan = { x: 0, y: 0 };
	onimageload?.(img.naturalWidth, img.naturalHeight);
}

function handleWheel(event: WheelEvent) {
	event.preventDefault();
	if (renderRect.w === 0) return;

	const next = clamp(zoom * Math.exp(-event.deltaY * 0.002), ZOOM_MIN, ZOOM_MAX);
	if (next === zoom) return;

	const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
	const cursorX = event.clientX - rect.left;
	const cursorY = event.clientY - rect.top;
	const normX = (cursorX - renderRect.x) / renderRect.w;
	const normY = (cursorY - renderRect.y) / renderRect.h;

	const w = renderRect.w * (next / zoom);
	const h = renderRect.h * (next / zoom);

	zoom = next;
	pan = clampPan(cursorX - normX * w - (containerSize.w - w) / 2, cursorY - normY * h - (containerSize.h - h) / 2);
}

function handlePointerDown(event: PointerEvent) {
	if (event.button !== 0) return;
	dragOrigin = { x: event.clientX, y: event.clientY, panX: pan.x, panY: pan.y };
	dragMoved = false;
}

function handlePointerMove(event: PointerEvent) {
	if (!dragOrigin) return;
	if (event.buttons === 0) {
		dragOrigin = null;
		return;
	}

	const dx = event.clientX - dragOrigin.x;
	const dy = event.clientY - dragOrigin.y;

	if (!dragMoved) {
		if (Math.abs(dx) < DRAG_THRESHOLD && Math.abs(dy) < DRAG_THRESHOLD) return;
		dragMoved = true;
		panning = pannable;
		if (panning) (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
	}

	if (panning) pan = clampPan(dragOrigin.panX + dx, dragOrigin.panY + dy);
}

function handlePointerUp(event: PointerEvent) {
	const node = event.currentTarget as HTMLElement;
	if (node.hasPointerCapture(event.pointerId)) node.releasePointerCapture(event.pointerId);
	panning = false;
	dragOrigin = null;
}

function selectPoint(pointId: number) {
	onselectpoint?.(pointId);
}

// Frames overlap heavily, so their fills are not hit-testable in the DOM (only the
// selected frame is, for dragging). Every other click on the overlay is resolved here
// against frame geometry: a click inside a stack steps to the next frame under the
// cursor, so a buried frame is reachable by clicking the same spot again.
function handleOverlayClick(event: PointerEvent) {
	if (dragMoved || renderRect.w === 0 || renderRect.h === 0) return;

	const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
	const normX = clamp((event.clientX - rect.left) / rect.width, 0, 1);
	const normY = clamp((event.clientY - rect.top) / rect.height, 0, 1);

	const hits = framesAtPoint(frames, normX, normY);
	if (hits.length === 0) {
		oncanvasclick?.(normX, normY, event.altKey);
		return;
	}

	if (event.shiftKey || event.metaKey) {
		const target = hits.find((f) => f.id === selectedFrameId) ?? hits[0];
		ontoggleselect?.(target.id);
		return;
	}

	onselectframe?.(cycleFrameId(hits, selectedFrameId, 1));
}

function isInputFocused() {
	const el = document.activeElement;
	if (!el) return false;
	const tag = el.tagName.toLowerCase();
	return tag === 'input' || tag === 'textarea' || (el as HTMLElement).isContentEditable;
}

function handleKeyDown(event: KeyboardEvent) {
	if (event.metaKey || event.ctrlKey || event.altKey || isInputFocused()) return;

	if (event.key === '0') {
		event.preventDefault();
		fitMode = fitMode === 'fit' ? 'fill' : 'fit';
		zoom = 1;
		pan = { x: 0, y: 0 };
	} else if (event.key >= '1' && event.key <= '9') {
		const index = parseInt(event.key, 10) - 1;
		if (index < frames.length) {
			event.preventDefault();
			onselectframe?.(frames[index].id);
		}
	}
}
</script>

<svelte:window onkeydown={handleKeyDown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="canvas-wrapper"
	class:panning
	class:pannable
	{@attach observeContainer}
	onwheel={handleWheel}
	onpointerdown={handlePointerDown}
	onpointermove={handlePointerMove}
	onpointerup={handlePointerUp}
	onpointercancel={handlePointerUp}
>
	<img
		src={imageSrc}
		alt={imageName || 'Photo'}
		draggable={false}
		onload={handleImageLoad}
		class="photo"
		style="left:{renderRect.x}px;top:{renderRect.y}px;width:{renderRect.w}px;height:{renderRect.h}px;"
	/>

	{#if renderRect.w > 0}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="overlay-container"
			style="left:{renderRect.x}px;top:{renderRect.y}px;width:{renderRect.w}px;height:{renderRect.h}px;"
			onpointerup={handleOverlayClick}
		>
		{#each frames as frame, i (frame.id)}
			<ResizableFrame
				{frame}
				index={i}
				imageWidth={renderRect.w}
				imageHeight={renderRect.h}
				scale={1}
				selected={selectedFrameId === frame.id}
				inSelection={!hasSelection || selection.has(frame.id)}
				compositionLabel={compositionLabels.get(frame.id) ?? ''}
				clipsRequired={clipsRequiredPoints(frame, frame.id, focalPoints)}
				onclaim={(geometry) => onclaimframe?.(frame.id, geometry)}
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
					onmove={(position) => onmovepoint?.(point.id, position)}
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
		touch-action: none;
	}

	.canvas-wrapper.pannable {
		cursor: grab;
	}

	.canvas-wrapper.panning {
		cursor: grabbing;
	}

	.photo {
		position: absolute;
		display: block;
		user-select: none;
		pointer-events: none;
		max-width: none;
		max-height: none;
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
