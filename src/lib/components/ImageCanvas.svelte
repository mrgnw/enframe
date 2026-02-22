<script lang="ts">
import type { FocalPoint, Frame, Photo } from '$lib/db';
import { getImageSrc } from '$lib/tauri';
import DraggableFocalPoint from './DraggableFocalPoint.svelte';
import ResizableFrame from './ResizableFrame.svelte';

interface Props {
	photo: Photo;
	frames?: Frame[];
	focalPoints?: FocalPoint[];
	onchange?: () => void;
	ondeleteframe?: (frameId: number) => void;
}

let { photo, frames = $bindable([]), focalPoints = $bindable([]), onchange, ondeleteframe }: Props = $props();

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
	if (containerEl) {
		resizeObserver?.disconnect();
		resizeObserver = new ResizeObserver(() => computeRenderRect());
		resizeObserver.observe(containerEl);
	}
}

let resizeObserver: ResizeObserver | undefined;

let selectedFrameId = $state<number | null>(null);

function selectFrame(frameId: number) {
	selectedFrameId = frameId;
}

function handleKeyDown(event: KeyboardEvent) {
	if (event.key >= '1' && event.key <= '9') {
		const index = parseInt(event.key) - 1;
		if (index < frames.length) {
			event.preventDefault();
			selectedFrameId = frames[index].id;
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
		<div
			class="overlay-container"
			style="left:{renderRect.x}px;top:{renderRect.y}px;width:{renderRect.w}px;height:{renderRect.h}px;"
		>
			{#each frames as frame (frame.id)}
				<ResizableFrame
					{frame}
					imageWidth={renderRect.w}
					imageHeight={renderRect.h}
					scale={1}
					selected={selectedFrameId === frame.id}
					onSelect={() => selectFrame(frame.id)}
					{onchange}
				/>
			{/each}

			{#each focalPoints as point (point.id)}
				<DraggableFocalPoint
					{point}
					imageWidth={renderRect.w}
					imageHeight={renderRect.h}
					scale={1}
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
		pointer-events: none;
	}
</style>
