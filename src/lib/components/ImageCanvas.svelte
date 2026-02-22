<script lang="ts">
	import type { Photo, Frame, FocalPoint } from '$lib/db';
	import { getImageSrc } from '$lib/tauri';
	import DraggableFocalPoint from './DraggableFocalPoint.svelte';
	import ResizableFrame from './ResizableFrame.svelte';

	interface Props {
		photo: Photo;
		frames?: Frame[];
		focalPoints?: FocalPoint[];
		onUpdate?: () => void;
	}

	let { photo, frames = [], focalPoints = [], onUpdate }: Props = $props();

	let containerEl = $state<HTMLDivElement | null>(null);
	let imgEl = $state<HTMLImageElement | null>(null);
	let scale = $state(1);
	let translateX = $state(0);
	let translateY = $state(0);
	let isPanning = $state(false);
	let panStart = $state({ x: 0, y: 0 });

	function handleWheel(event: WheelEvent) {
		event.preventDefault();
		const delta = event.deltaY * -0.001;
		const newScale = Math.max(0.1, Math.min(5, scale + delta));
		scale = newScale;
	}

	function handlePointerDown(event: PointerEvent) {
		if (event.button !== 0) return;
		isPanning = true;
		panStart = { x: event.clientX - translateX, y: event.clientY - translateY };
		(event.target as HTMLElement).setPointerCapture(event.pointerId);
	}

	function handlePointerMove(event: PointerEvent) {
		if (!isPanning) return;
		translateX = event.clientX - panStart.x;
		translateY = event.clientY - panStart.y;
	}

	function handlePointerUp(event: PointerEvent) {
		isPanning = false;
		(event.target as HTMLElement).releasePointerCapture(event.pointerId);
	}

	function resetView() {
		scale = 1;
		translateX = 0;
		translateY = 0;
	}
</script>

<div class="canvas-wrapper">
	<div class="controls">
		<button onclick={() => scale = Math.min(5, scale + 0.1)}>+</button>
		<button onclick={() => scale = Math.max(0.1, scale - 0.1)}>−</button>
		<button onclick={resetView}>Reset</button>
		<span class="zoom-level">{Math.round(scale * 100)}%</span>
	</div>

	<div
		bind:this={containerEl}
		class="canvas-container"
		role="img"
		aria-label="Zoomable image canvas"
		onwheel={handleWheel}
		onpointerdown={handlePointerDown}
		onpointermove={handlePointerMove}
		onpointerup={handlePointerUp}
		style="cursor: {isPanning ? 'grabbing' : 'grab'}"
	>
		<div
			class="canvas-content"
			style="transform: translate({translateX}px, {translateY}px) scale({scale})"
		>
			<img
				bind:this={imgEl}
				src={getImageSrc(photo.file_path)}
				alt={photo.title || 'Photo'}
				draggable={false}
			/>

			<!-- Frame overlays -->
			{#if imgEl && frames.length > 0}
				{#each frames as frame (frame.id)}
					<ResizableFrame
						{frame}
						imageWidth={imgEl.naturalWidth}
						imageHeight={imgEl.naturalHeight}
						{scale}
						{onUpdate}
					/>
				{/each}
			{/if}

			<!-- Focal point overlays -->
			{#if imgEl && focalPoints.length > 0}
				{#each focalPoints as point (point.id)}
					<DraggableFocalPoint
						{point}
						imageWidth={imgEl.naturalWidth}
						imageHeight={imgEl.naturalHeight}
						{scale}
						{onUpdate}
					/>
				{/each}
			{/if}
		</div>
	</div>
</div>

<style>
	.canvas-wrapper {
		position: relative;
		width: 100%;
		height: 100%;
		overflow: hidden;
	}

	.controls {
		position: absolute;
		top: 1rem;
		right: 1rem;
		display: flex;
		gap: 0.5rem;
		z-index: 10;
		background: white;
		padding: 0.5rem;
		border-radius: 0.375rem;
		box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
	}

	.controls button {
		width: 2rem;
		height: 2rem;
		padding: 0;
		background: #f3f4f6;
		color: #374151;
		border: 1px solid #d1d5db;
		border-radius: 0.25rem;
		cursor: pointer;
		font-size: 1.125rem;
		font-weight: 600;
		line-height: 1;
	}

	.controls button:hover {
		background: #e5e7eb;
	}

	.zoom-level {
		display: flex;
		align-items: center;
		padding: 0 0.5rem;
		font-size: 0.875rem;
		color: #6b7280;
	}

	.canvas-container {
		width: 100%;
		height: 100%;
		overflow: hidden;
		touch-action: none;
	}

	.canvas-content {
		position: relative;
		transform-origin: center center;
		transition: transform 0.05s ease-out;
		will-change: transform;
	}

	.canvas-content img {
		display: block;
		max-width: 100%;
		height: auto;
		user-select: none;
	}


</style>
