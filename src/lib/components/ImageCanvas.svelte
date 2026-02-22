<script lang="ts">
import type { FocalPoint, Frame, Photo } from '$lib/db';
import { getImageSrc } from '$lib/tauri';
import DraggableFocalPoint from './DraggableFocalPoint.svelte';
import ResizableFrame from './ResizableFrame.svelte';

interface Props {
	photo: Photo;
	frames?: Frame[];
	focalPoints?: FocalPoint[];
}

let { photo, frames = $bindable([]), focalPoints = $bindable([]) }: Props = $props();

let containerEl = $state<HTMLDivElement | null>(null);
let imgEl = $state<HTMLImageElement | null>(null);
let scale = $state(1);
let translateX = $state(0);
let translateY = $state(0);
let isPanning = $state(false);
let panStart = $state({ x: 0, y: 0 });
let clickStart = $state<{ x: number; y: number } | null>(null);
let fitMode = $state<'fit' | 'fill'>('fit');
let selectedFrameId = $state<number | null>(null);
let showSidebar = $state(true);

function getFramesUnderCursor(clientX: number, clientY: number): number[] {
	if (!containerEl || !imgEl) return [];

	const rect = containerEl.getBoundingClientRect();
	const mouseX = clientX - rect.left;
	const mouseY = clientY - rect.top;

	// Convert to image coordinates (0-1)
	const imgX = (mouseX - translateX) / scale / imgEl.naturalWidth;
	const imgY = (mouseY - translateY) / scale / imgEl.naturalHeight;

	return frames
		.map((f, i) => ({ frame: f, index: i }))
		.filter(
			({ frame }) =>
				imgX >= frame.x && imgX <= frame.x + frame.width && imgY >= frame.y && imgY <= frame.y + frame.height
		)
		.map(({ index }) => index);
}

function handleWheel(event: WheelEvent) {
	event.preventDefault();
	if (!containerEl) return;

	// Cmd/Ctrl + scroll = cycle frames under cursor
	if (event.metaKey || event.ctrlKey) {
		const framesUnder = getFramesUnderCursor(event.clientX, event.clientY);
		if (framesUnder.length === 0) return;

		const currentIndex = selectedFrameId ? framesUnder.indexOf(frames.findIndex((f) => f.id === selectedFrameId)) : -1;
		const direction = event.deltaY > 0 ? 1 : -1;
		const nextIndex = (currentIndex + direction + framesUnder.length) % framesUnder.length;

		selectedFrameId = frames[framesUnder[nextIndex]].id;
		return;
	}

	const delta = event.deltaY * -0.001;
	const newScale = Math.max(0.1, Math.min(5, scale + delta));

	// Zoom centered on cursor position
	const rect = containerEl.getBoundingClientRect();
	const mouseX = event.clientX - rect.left;
	const mouseY = event.clientY - rect.top;

	// Calculate the point under the cursor in the scaled coordinate space
	const pointX = (mouseX - translateX) / scale;
	const pointY = (mouseY - translateY) / scale;

	// Adjust translation to keep the same point under the cursor
	translateX = mouseX - pointX * newScale;
	translateY = mouseY - pointY * newScale;

	scale = newScale;
}

function handlePointerDown(event: PointerEvent) {
	if (event.button !== 0) return;
	clickStart = { x: event.clientX, y: event.clientY };
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
	// Check if this was a click (minimal movement)
	if (clickStart) {
		const dx = Math.abs(event.clientX - clickStart.x);
		const dy = Math.abs(event.clientY - clickStart.y);

		if (dx < 5 && dy < 5) {
			// This was a click - cycle frames under cursor
			const framesUnder = getFramesUnderCursor(event.clientX, event.clientY);
			if (framesUnder.length > 1) {
				const currentIndex = selectedFrameId
					? framesUnder.indexOf(frames.findIndex((f) => f.id === selectedFrameId))
					: -1;
				const nextIndex = (currentIndex + 1) % framesUnder.length;
				selectedFrameId = frames[framesUnder[nextIndex]].id;
			} else if (framesUnder.length === 1) {
				selectedFrameId = frames[framesUnder[0]].id;
			}
		}
	}

	clickStart = null;
	isPanning = false;
	(event.target as HTMLElement).releasePointerCapture(event.pointerId);
}

function calculateFitScale(): number {
	if (!containerEl || !imgEl) return 1;
	const containerWidth = containerEl.clientWidth;
	const containerHeight = containerEl.clientHeight;
	const imgWidth = imgEl.naturalWidth;
	const imgHeight = imgEl.naturalHeight;

	if (!imgWidth || !imgHeight) return 1;

	const scaleX = containerWidth / imgWidth;
	const scaleY = containerHeight / imgHeight;

	return fitMode === 'fit' ? Math.min(scaleX, scaleY) : Math.max(scaleX, scaleY);
}

function fitToView() {
	scale = calculateFitScale();
	translateX = 0;
	translateY = 0;
}

function resetView() {
	fitToView();
}

function toggleFitMode() {
	fitMode = fitMode === 'fit' ? 'fill' : 'fit';
	fitToView();
}

function handleImageLoad() {
	fitToView();
}

function handleKeyDown(event: KeyboardEvent) {
	if (event.key === '0') {
		event.preventDefault();
		toggleFitMode();
	} else if (event.key >= '1' && event.key <= '9') {
		const index = parseInt(event.key) - 1;
		if (index < frames.length) {
			event.preventDefault();
			selectedFrameId = frames[index].id;
		}
	}
}

function selectFrame(frameId: number) {
	selectedFrameId = frameId;
}

function toggleSidebar() {
	showSidebar = !showSidebar;
}
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="canvas-wrapper">
	{#if showSidebar && frames.length > 0}
		<div class="sidebar">
			<div class="sidebar-header">
				<h3>Frames</h3>
				<button class="sidebar-toggle" onclick={toggleSidebar}>×</button>
			</div>
			<div class="frame-list">
				{#each frames as frame, i (frame.id)}
					<button
						class="frame-item"
						class:selected={selectedFrameId === frame.id}
						onclick={() => selectFrame(frame.id)}
					>
						<span class="frame-number">{i + 1}</span>
						<span class="frame-label">{frame.label || 'Frame ' + (i + 1)}</span>
						<span class="frame-dims">{Math.round(frame.width * 100)}% × {Math.round(frame.height * 100)}%</span>
					</button>
				{/each}
			</div>
		</div>
	{:else if frames.length > 0}
		<button class="sidebar-toggle-btn" onclick={toggleSidebar}>☰ Frames</button>
	{/if}

	<div class="controls">
		<button onclick={() => scale = Math.min(5, scale + 0.1)}>+</button>
		<button onclick={() => scale = Math.max(0.1, scale - 0.1)}>−</button>
		<button onclick={resetView}>Fit</button>
		<span class="zoom-level">{Math.round(scale * 100)}% ({fitMode})</span>
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
				onload={handleImageLoad}
			/>

			<!-- Frame overlays -->
			{#if imgEl && frames.length > 0}
				{#each frames as frame (frame.id)}
					<ResizableFrame
						{frame}
						imageWidth={imgEl.naturalWidth}
						imageHeight={imgEl.naturalHeight}
						{scale}
						selected={selectedFrameId === frame.id}
						onSelect={() => selectFrame(frame.id)}
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

	.sidebar {
		position: absolute;
		left: 1rem;
		top: 1rem;
		bottom: 1rem;
		width: 240px;
		background: white;
		border-radius: 0.5rem;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
		display: flex;
		flex-direction: column;
		z-index: 10;
		overflow: hidden;
	}

	.sidebar-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 1rem;
		border-bottom: 1px solid #e5e7eb;
	}

	.sidebar-header h3 {
		margin: 0;
		font-size: 1rem;
		font-weight: 600;
		color: #111827;
	}

	.sidebar-toggle {
		width: 2rem;
		height: 2rem;
		padding: 0;
		background: transparent;
		border: none;
		font-size: 1.5rem;
		line-height: 1;
		cursor: pointer;
		color: #6b7280;
	}

	.sidebar-toggle:hover {
		color: #111827;
	}

	.sidebar-toggle-btn {
		position: absolute;
		left: 1rem;
		top: 1rem;
		padding: 0.5rem 1rem;
		background: white;
		border: 1px solid #d1d5db;
		border-radius: 0.375rem;
		font-size: 0.875rem;
		cursor: pointer;
		box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
		z-index: 10;
	}

	.sidebar-toggle-btn:hover {
		background: #f9fafb;
	}

	.frame-list {
		flex: 1;
		overflow-y: auto;
		padding: 0.5rem;
	}

	.frame-item {
		width: 100%;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.75rem;
		margin-bottom: 0.5rem;
		background: #f9fafb;
		border: 1px solid #e5e7eb;
		border-radius: 0.375rem;
		cursor: pointer;
		text-align: left;
		transition: all 0.15s;
	}

	.frame-item:hover {
		background: #f3f4f6;
		border-color: #d1d5db;
	}

	.frame-item.selected {
		background: #fef3c7;
		border-color: #f59e0b;
	}

	.frame-number {
		flex-shrink: 0;
		width: 1.5rem;
		height: 1.5rem;
		display: flex;
		align-items: center;
		justify-content: center;
		background: white;
		border: 1px solid #d1d5db;
		border-radius: 0.25rem;
		font-size: 0.75rem;
		font-weight: 600;
		color: #6b7280;
	}

	.frame-item.selected .frame-number {
		background: #f59e0b;
		border-color: #f59e0b;
		color: white;
	}

	.frame-label {
		flex: 1;
		font-size: 0.875rem;
		font-weight: 500;
		color: #111827;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.frame-dims {
		flex-shrink: 0;
		font-size: 0.75rem;
		color: #6b7280;
	}
</style>
