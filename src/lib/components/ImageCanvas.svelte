<script lang="ts">
import type { FocalPoint, Frame, Photo } from '$lib/db';
import { exportFrames, exportMetadata, getDefaultExportDir, pickDirectory } from '$lib/export';
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
let aspectLocks = $state<Map<number, boolean>>(new Map());

// Export state
let exportChecked = $state<Set<number>>(new Set());
let exportDir = $state(getDefaultExportDir(photo.file_path));
let isSourceDir = $state(true);
let exporting = $state(false);
let exportProgress = $state({ current: 0, total: 0 });

// Initialize exportChecked with all frame IDs when frames change
$effect(() => {
	const currentIds = new Set(frames.map((f) => f.id));
	// Add any new frame IDs
	for (const id of currentIds) {
		if (!exportChecked.has(id)) {
			exportChecked.add(id);
			exportChecked = new Set(exportChecked);
		}
	}
});

const commonRatios = [
	{ label: '16:9', value: 16 / 9 },
	{ label: '4:3', value: 4 / 3 },
	{ label: '1:1', value: 1 },
	{ label: '3:2', value: 3 / 2 },
	{ label: '2:3', value: 2 / 3 },
	{ label: '9:16', value: 9 / 16 }
];

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
	if (!containerEl || !imgEl) return;

	scale = calculateFitScale();

	// Center the image in the container
	const containerWidth = containerEl.clientWidth;
	const containerHeight = containerEl.clientHeight;
	const scaledWidth = imgEl.naturalWidth * scale;
	const scaledHeight = imgEl.naturalHeight * scale;

	translateX = (containerWidth - scaledWidth) / 2;
	translateY = (containerHeight - scaledHeight) / 2;
}

function resetView() {
	fitToView();
}

function fitToFrame() {
	if (!containerEl || !imgEl || !selectedFrameId) return;

	const frame = frames.find((f) => f.id === selectedFrameId);
	if (!frame) return;

	const containerWidth = containerEl.clientWidth;
	const containerHeight = containerEl.clientHeight;

	// Calculate frame dimensions in pixels
	const frameWidth = frame.width * imgEl.naturalWidth;
	const frameHeight = frame.height * imgEl.naturalHeight;

	// Calculate scale to fit frame in container
	const scaleX = containerWidth / frameWidth;
	const scaleY = containerHeight / frameHeight;
	scale = Math.min(scaleX, scaleY) * 0.9; // 90% to add padding

	// Calculate frame center in pixels
	const frameCenterX = (frame.x + frame.width / 2) * imgEl.naturalWidth;
	const frameCenterY = (frame.y + frame.height / 2) * imgEl.naturalHeight;

	// Center frame in container
	translateX = containerWidth / 2 - frameCenterX * scale;
	translateY = containerHeight / 2 - frameCenterY * scale;
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

function toggleAspectLock(frameId: number) {
	const current = aspectLocks.get(frameId) || false;
	aspectLocks.set(frameId, !current);
	aspectLocks = new Map(aspectLocks); // Trigger reactivity
}

function applyRatio(frameId: number, ratio: number) {
	const frame = frames.find((f) => f.id === frameId);
	if (!frame) return;

	// Keep the center position, adjust size to match ratio
	const centerX = frame.x + frame.width / 2;
	const centerY = frame.y + frame.height / 2;

	let newW = frame.width;
	let newH = frame.width / ratio;

	// If height would exceed bounds, scale down based on height instead
	if (newH > 1) {
		newH = frame.height;
		newW = frame.height * ratio;
	}

	frame.width = Math.min(1, newW);
	frame.height = Math.min(1, newH);
	frame.x = Math.max(0, Math.min(1 - frame.width, centerX - frame.width / 2));
	frame.y = Math.max(0, Math.min(1 - frame.height, centerY - frame.height / 2));

	// Auto-lock aspect when applying a ratio
	aspectLocks.set(frameId, true);
	aspectLocks = new Map(aspectLocks);
}

// Export functions
function toggleExportCheck(frameId: number) {
	if (exportChecked.has(frameId)) {
		exportChecked.delete(frameId);
	} else {
		exportChecked.add(frameId);
	}
	exportChecked = new Set(exportChecked);
}

function toggleAllExport() {
	if (exportChecked.size === frames.length) {
		exportChecked = new Set();
	} else {
		exportChecked = new Set(frames.map((f) => f.id));
	}
}

async function handlePickDir() {
	const dir = await pickDirectory();
	if (dir) {
		exportDir = dir;
		isSourceDir = false;
	}
}

async function handleExport() {
	const framesToExport = frames.filter((f) => exportChecked.has(f.id));
	if (framesToExport.length === 0) return;

	exporting = true;
	exportProgress = { current: 0, total: framesToExport.length };

	try {
		const paths = await exportFrames(photo, framesToExport, exportDir, 0.92, (current, total) => {
			exportProgress = { current, total };
		});

		// Also export metadata
		const basename = photo.title?.replace(/\.[^/.]+$/, '').replace(/\s+/g, '-') || 'photo';
		await exportMetadata(photo, framesToExport, focalPoints, `${exportDir}/${basename}.metadata.json`);
	} catch (error) {
		console.error('Export failed:', error);
	} finally {
		exporting = false;
	}
}
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="canvas-wrapper">
	{#if showSidebar && frames.length > 0}
		<div class="sidebar">
			<div class="sidebar-header">
				<label class="export-all-check" title={exportChecked.size === frames.length ? 'Deselect all for export' : 'Select all for export'}>
					<input
						type="checkbox"
						checked={exportChecked.size === frames.length}
						indeterminate={exportChecked.size > 0 && exportChecked.size < frames.length}
						onchange={toggleAllExport}
					/>
				</label>
				<h3>Frames</h3>
				<button class="sidebar-toggle" onclick={toggleSidebar}>×</button>
			</div>
			<div class="frame-list">
				{#each frames as frame, i (frame.id)}
					<div
						class="frame-item-wrapper"
						class:editing={selectedFrameId === frame.id}
						class:export-checked={exportChecked.has(frame.id)}
						class:export-unchecked={!exportChecked.has(frame.id)}
					>
						<div class="frame-item-row">
							<label class="export-check" onclick={(e: MouseEvent) => e.stopPropagation()}>
								<input
									type="checkbox"
									checked={exportChecked.has(frame.id)}
									onchange={() => toggleExportCheck(frame.id)}
								/>
							</label>
							<button
								class="frame-item"
								onclick={() => selectFrame(frame.id)}
							>
								<span class="frame-number">{i + 1}</span>
								<span class="frame-label">{frame.label || 'Frame ' + (i + 1)}</span>
								<span class="frame-dims">{Math.round(frame.width * 100)}% × {Math.round(frame.height * 100)}%</span>
							</button>
						</div>
						
						{#if selectedFrameId === frame.id}
							<div class="frame-controls">
								<button
									class="aspect-lock-btn"
									class:locked={aspectLocks.get(frame.id)}
									onclick={(e) => { e.stopPropagation(); toggleAspectLock(frame.id); }}
									title={aspectLocks.get(frame.id) ? 'Unlock aspect ratio' : 'Lock aspect ratio'}
								>
									{aspectLocks.get(frame.id) ? '🔒' : '🔓'}
								</button>
								
								<div class="ratio-picker">
									{#each commonRatios as { label, value }}
										<button
											class="ratio-btn"
											onclick={(e) => { e.stopPropagation(); applyRatio(frame.id, value); }}
											title="Set to {label} ratio"
										>
											{label}
										</button>
									{/each}
								</div>
							</div>
						{/if}
					</div>
				{/each}
			</div>
			<div class="sidebar-footer">
				<button class="dir-picker" onclick={handlePickDir} title="Change export directory">
					<span class="dir-icon">{isSourceDir ? '📁' : '📂'}</span>
					<span class="dir-path">{exportDir.split('/').slice(-2).join('/')}</span>
				</button>
				<button
					class="export-btn"
					onclick={handleExport}
					disabled={exporting || exportChecked.size === 0}
				>
					{#if exporting}
						Exporting {exportProgress.current}/{exportProgress.total}...
					{:else}
						Export {exportChecked.size} frame{exportChecked.size === 1 ? '' : 's'}
					{/if}
				</button>
			</div>
		</div>
	{:else if frames.length > 0}
		<button class="sidebar-toggle-btn" onclick={toggleSidebar}>☰ Frames</button>
	{/if}

	<div class="controls">
		<button onclick={() => scale = Math.min(5, scale + 0.1)}>+</button>
		<button onclick={() => scale = Math.max(0.1, scale - 0.1)}>−</button>
		<button onclick={resetView}>Fit</button>
		{#if selectedFrameId}
			<button onclick={fitToFrame} title="Fit to selected frame">Frame</button>
		{/if}
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
						exportChecked={exportChecked.has(frame.id)}
						aspectLocked={aspectLocks.get(frame.id) || false}
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
		gap: 0.5rem;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid #e5e7eb;
	}

	.sidebar-header h3 {
		margin: 0;
		font-size: 1rem;
		font-weight: 600;
		color: #111827;
		flex: 1;
	}

	.export-all-check {
		display: flex;
		align-items: center;
		cursor: pointer;
	}

	.export-all-check input {
		width: 1rem;
		height: 1rem;
		cursor: pointer;
		accent-color: #3b82f6;
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

	.frame-item-wrapper {
		margin-bottom: 0.5rem;
		border-radius: 0.375rem;
		border: 1px solid transparent;
		transition: all 0.15s;
	}

	/* Export-checked: blue left border accent */
	.frame-item-wrapper.export-checked {
		border-left: 3px solid #3b82f6;
	}

	/* Export-unchecked: muted */
	.frame-item-wrapper.export-unchecked {
		opacity: 0.5;
	}

	/* Editing: orange background */
	.frame-item-wrapper.editing {
		background: #fef3c7;
		border-color: #f59e0b;
		border-left: 3px solid #f59e0b;
		padding: 0.5rem;
		margin-bottom: 0.75rem;
		opacity: 1;
	}

	.frame-item-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.export-check {
		display: flex;
		align-items: center;
		cursor: pointer;
		flex-shrink: 0;
	}

	.export-check input {
		width: 0.875rem;
		height: 0.875rem;
		cursor: pointer;
		accent-color: #3b82f6;
	}

	.frame-item {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.75rem;
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

	.frame-item-wrapper.editing .frame-item {
		background: white;
		border-color: transparent;
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

	.frame-item-wrapper.editing .frame-number {
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

	.frame-controls {
		margin-top: 0.5rem;
		padding-top: 0.5rem;
		border-top: 1px solid #f59e0b;
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}

	.aspect-lock-btn {
		flex-shrink: 0;
		width: 2rem;
		height: 2rem;
		padding: 0;
		background: white;
		border: 1px solid #d1d5db;
		border-radius: 0.25rem;
		font-size: 1rem;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.aspect-lock-btn:hover {
		background: #f9fafb;
	}

	.aspect-lock-btn.locked {
		background: #fef3c7;
		border-color: #f59e0b;
	}

	.ratio-picker {
		flex: 1;
		display: flex;
		gap: 0.25rem;
		flex-wrap: wrap;
	}

	.ratio-btn {
		padding: 0.25rem 0.5rem;
		background: white;
		border: 1px solid #d1d5db;
		border-radius: 0.25rem;
		font-size: 0.75rem;
		cursor: pointer;
		transition: all 0.15s;
	}

	.ratio-btn:hover {
		background: #f9fafb;
		border-color: #9ca3af;
	}

	.sidebar-footer {
		padding: 0.75rem;
		border-top: 1px solid #e5e7eb;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.dir-picker {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem;
		background: #f9fafb;
		border: 1px solid #e5e7eb;
		border-radius: 0.375rem;
		cursor: pointer;
		text-align: left;
		width: 100%;
		transition: all 0.15s;
	}

	.dir-picker:hover {
		background: #f3f4f6;
		border-color: #d1d5db;
	}

	.dir-icon {
		flex-shrink: 0;
		font-size: 1rem;
	}

	.dir-path {
		flex: 1;
		font-size: 0.75rem;
		color: #6b7280;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.export-btn {
		width: 100%;
		padding: 0.625rem 1rem;
		background: #3b82f6;
		color: white;
		border: none;
		border-radius: 0.375rem;
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		transition: background 0.15s;
	}

	.export-btn:hover:not(:disabled) {
		background: #2563eb;
	}

	.export-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
