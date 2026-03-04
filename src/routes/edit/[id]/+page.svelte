<script lang="ts">
import { onMount } from 'svelte';
import { toast } from 'svelte-sonner';

function yieldFrame(): Promise<void> {
	return new Promise((r) => requestAnimationFrame(() => r()));
}

import { goto } from '$app/navigation';
import { page } from '$app/stores';
import CommandBar from '$lib/components/CommandBar.svelte';
import CursorProgress from '$lib/components/CursorProgress.svelte';
import FloatingToolbar from '$lib/components/FloatingToolbar.svelte';
import FramePointPanel from '$lib/components/FramePointPanel.svelte';
import ImageCanvas from '$lib/components/ImageCanvas.svelte';
import PreviewPanel from '$lib/components/PreviewPanel.svelte';
import {
	AUTO_LANDSCAPE_LABEL,
	AUTO_PORTRAIT_LABEL,
	computeAutoFrame,
	computeFrameLabel,
	isAutoFrame
} from '$lib/composition';
import {
	addFocalPoint,
	addFrame,
	deleteFocalPoint,
	deleteFrame,
	deletePhoto,
	type FocalPoint,
	type Frame,
	getFocalPoints,
	getFrames,
	getPhoto,
	type Photo,
	updateFocalPoint,
	updateFrame
} from '$lib/db';
import { type ExportFormat, exportFrames, exportMetadata, getDefaultExportDir, pickDirectory } from '$lib/export';

type HistoryState = {
	frames: Frame[];
	focalPoints: FocalPoint[];
};

let photo = $state<Photo | null>(null);
let frames = $state<Frame[]>([]);
let focalPoints = $state<FocalPoint[]>([]);
let loading = $state(true);

let saveStatus = $state<'saved' | 'saving' | 'unsaved'>('saved');
let saveProgress = $state(0);
let saveTotal = $state(0);

const photoId = Number($page.params.id);

let history = $state<HistoryState[]>([]);
let historyIndex = $state(-1);
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

let exportDir = $state('');
let exportFormat = $state<ExportFormat>('avif');
let exporting = $state(false);
let exportProgress = $state(0);
let exportTotal = $state(0);

let selectedFrameId = $state<number | null>(null);
let selectedPointId = $state<number | null>(null);
let panelExpanded = $state(false);
let previewOpen = $state(false);
let previewMode = $state<'single' | 'all'>('all');
let imageWidth = $state(0);
let imageHeight = $state(0);

let mouseX = $state(0);
let mouseY = $state(0);
let mouseKnown = $state(false);
let compositionLabels = $state<Map<number, string>>(new Map());

const GUIDE_TYPES = ['off', 'thirds', 'phi', 'spiral', 'triangle', 'diagonal'] as const;
type GuideType = (typeof GUIDE_TYPES)[number];
let activeGuide = $state<GuideType>('off');

function handlePointerMove(event: PointerEvent) {
	mouseX = event.clientX;
	mouseY = event.clientY;
	mouseKnown = true;
}

let isBusy = $derived(saveStatus === 'saving' || exporting);
let busyProgress = $derived(
	exporting && exportTotal > 0
		? exportProgress / exportTotal
		: saveStatus === 'saving' && saveTotal > 0
			? saveProgress / saveTotal
			: 0
);
let busyColor = $derived(exporting ? '#22c55e' : '#3b82f6');
let progressX = $derived(mouseKnown ? mouseX : typeof window !== 'undefined' ? window.innerWidth / 2 : 0);
let progressY = $derived(mouseKnown ? mouseY : typeof window !== 'undefined' ? window.innerHeight / 2 : 0);

function captureSnapshot() {
	if (debounceTimer) clearTimeout(debounceTimer);
	debounceTimer = setTimeout(() => {
		history = history.slice(0, historyIndex + 1);
		history.push({
			frames: structuredClone(frames),
			focalPoints: structuredClone(focalPoints)
		});
		if (history.length > 50) {
			history = history.slice(-50);
		}
		historyIndex = history.length - 1;
		saveStatus = 'unsaved';
	}, 500);
}

function undo() {
	if (historyIndex <= 0) return;
	historyIndex--;
	const state = history[historyIndex];
	frames = structuredClone(state.frames);
	focalPoints = structuredClone(state.focalPoints);
}

function redo() {
	if (historyIndex >= history.length - 1) return;
	historyIndex++;
	const state = history[historyIndex];
	frames = structuredClone(state.frames);
	focalPoints = structuredClone(state.focalPoints);
}

function isInputFocused(): boolean {
	const el = document.activeElement;
	if (!el) return false;
	const tag = el.tagName.toLowerCase();
	return tag === 'input' || tag === 'textarea' || (el as HTMLElement).isContentEditable;
}

function handleDeselect() {
	selectedFrameId = null;
	selectedPointId = null;
}

function handleKeyDown(event: KeyboardEvent) {
	if ((event.metaKey || event.ctrlKey) && event.key === 'z') {
		event.preventDefault();
		if (event.shiftKey) {
			redo();
		} else {
			undo();
		}
	} else if ((event.metaKey || event.ctrlKey) && event.key === 's') {
		event.preventDefault();
		saveMetadata();
	} else if ((event.metaKey || event.ctrlKey) && event.key === 'e') {
		event.preventDefault();
		handleExport();
	} else if (event.key === 'Escape') {
		event.preventDefault();
		handleDeselect();
	} else if (!isInputFocused()) {
		if (event.key === ',') {
			event.preventDefault();
			selectPrevFrame();
		} else if (event.key === '.') {
			event.preventDefault();
			selectNextFrame();
		} else if (event.key === 'Backspace' || event.key === 'Delete') {
			if (!event.metaKey && !event.ctrlKey) {
				if (selectedFrameId != null) {
					event.preventDefault();
					handleDeleteFrame(selectedFrameId);
				} else if (selectedPointId != null) {
					event.preventDefault();
					handleDeletePoint(selectedPointId);
				}
			}
		} else if (event.key === 'f') {
			event.preventDefault();
			handleAddFrame();
		} else if (event.key === 'p') {
			event.preventDefault();
			handleAddFocalPoint();
		} else if (event.key === 'o') {
			event.preventDefault();
			handleSetOutputDir();
		} else if (event.key === 't') {
			event.preventDefault();
			panelExpanded = !panelExpanded;
		} else if (event.key === 'v') {
			event.preventDefault();
			previewOpen = !previewOpen;
		} else if (event.key === 'g') {
			event.preventDefault();
			const idx = GUIDE_TYPES.indexOf(activeGuide);
			activeGuide = GUIDE_TYPES[(idx + 1) % GUIDE_TYPES.length];
		}
	}
}

function selectNextFrame() {
	if (frames.length === 0) return;
	if (selectedFrameId == null) {
		selectedFrameId = frames[0].id;
		return;
	}
	const idx = frames.findIndex((f) => f.id === selectedFrameId);
	selectedFrameId = frames[(idx + 1) % frames.length].id;
}

function selectPrevFrame() {
	if (frames.length === 0) return;
	if (selectedFrameId == null) {
		selectedFrameId = frames[frames.length - 1].id;
		return;
	}
	const idx = frames.findIndex((f) => f.id === selectedFrameId);
	selectedFrameId = frames[(idx - 1 + frames.length) % frames.length].id;
}

function setExportFormat(format: ExportFormat) {
	exportFormat = format;
	toast.success(`Export format: ${format.toUpperCase()}`);
}

async function saveMetadata() {
	saveStatus = 'saving';
	saveProgress = 0;
	saveTotal = frames.length + focalPoints.length;
	await yieldFrame();
	try {
		for (const frame of frames) {
			await updateFrame(frame.id, {
				x: frame.x,
				y: frame.y,
				width: frame.width,
				height: frame.height,
				label: frame.label
			});
			saveProgress++;
			await yieldFrame();
		}
		for (const point of focalPoints) {
			await updateFocalPoint(point.id, {
				x: point.x,
				y: point.y,
				label: point.label,
				required: point.required
			});
			saveProgress++;
			await yieldFrame();
		}
		saveStatus = 'saved';
		toast.success('Saved');
	} catch (error) {
		console.error('Failed to save metadata:', error);
		saveStatus = 'unsaved';
		toast.error('Save failed');
	}
}

onMount(async () => {
	await loadPhoto();
	history.push({
		frames: structuredClone(frames),
		focalPoints: structuredClone(focalPoints)
	});
	historyIndex = 0;
});

function handleChange() {
	if (loading || history.length === 0) return;
	recomputeAutoFrames();
	captureSnapshot();
}

async function loadPhoto() {
	loading = true;
	try {
		photo = await getPhoto(photoId);
		frames = await getFrames(photoId);
		focalPoints = await getFocalPoints(photoId);
		if (photo) {
			exportDir = getDefaultExportDir(photo.file_path);
		}
	} catch (error) {
		console.error('Failed to load photo:', error);
	} finally {
		loading = false;
	}
}

async function handleAddFrame() {
	if (!photo) return;
	const id = await addFrame({
		photo_id: photoId,
		label: undefined,
		x: 0.2,
		y: 0.2,
		width: 0.6,
		height: 0.6,
		rotation: 0,
		is_default: false
	});
	await loadPhoto();
	selectedFrameId = id;
	panelExpanded = true;
}

async function handleAddFocalPoint() {
	if (!photo) return;
	await addFocalPoint({
		photo_id: photoId,
		frame_id: undefined,
		x: 0.5,
		y: 0.5,
		label: `Point ${focalPoints.length + 1}`,
		required: true
	});
	await loadPhoto();
}

async function handleDeleteFrame(frameId: number) {
	await deleteFrame(frameId);
	if (selectedFrameId === frameId) {
		selectedFrameId = null;
	}
	await loadPhoto();
	saveStatus = 'unsaved';
	captureSnapshot();
}

async function handleDeletePoint(pointId: number) {
	await deleteFocalPoint(pointId);
	if (selectedPointId === pointId) {
		selectedPointId = null;
	}
	await loadPhoto();
	await recomputeAutoFrames();
	saveStatus = 'unsaved';
	captureSnapshot();
}

async function handleCanvasClick(normX: number, normY: number, altKey: boolean) {
	if (!photo) return;

	// Add a global focal point at the clicked position
	await addFocalPoint({
		photo_id: photoId,
		frame_id: undefined,
		x: normX,
		y: normY,
		label: `Point ${focalPoints.length + 1}`,
		required: true
	});
	await loadPhoto();

	if (altKey) {
		// Ensure auto frame pair exists, then recompute
		await ensureAutoFrames();
	}

	await recomputeAutoFrames();
	handleChange();
}

async function ensureAutoFrames() {
	if (!photo) return;
	const hasAutoL = frames.some((f) => f.label === AUTO_LANDSCAPE_LABEL);
	const hasAutoP = frames.some((f) => f.label === AUTO_PORTRAIT_LABEL);

	if (!hasAutoL) {
		await addFrame({
			photo_id: photoId,
			label: AUTO_LANDSCAPE_LABEL,
			x: 0.1,
			y: 0.1,
			width: 0.8,
			height: 0.45,
			rotation: 0,
			is_default: false
		});
	}
	if (!hasAutoP) {
		await addFrame({
			photo_id: photoId,
			label: AUTO_PORTRAIT_LABEL,
			x: 0.3,
			y: 0.05,
			width: 0.4,
			height: 0.9,
			rotation: 0,
			is_default: false
		});
	}
	if (!hasAutoL || !hasAutoP) {
		await loadPhoto();
	}
}

async function recomputeAutoFrames() {
	if (!photo || imageWidth === 0 || imageHeight === 0) return;

	// Use all global focal points (frame_id is null/undefined)
	const globalPoints = focalPoints.filter((fp) => !fp.frame_id).map((fp) => ({ x: fp.x, y: fp.y }));

	const imgAspect = imageWidth / imageHeight;

	const autoL = frames.find((f) => f.label === AUTO_LANDSCAPE_LABEL);
	const autoP = frames.find((f) => f.label === AUTO_PORTRAIT_LABEL);

	if (globalPoints.length > 0) {
		if (autoL) {
			const result = computeAutoFrame(globalPoints, 16 / 9, imgAspect);
			if (result) {
				autoL.x = result.rect.x;
				autoL.y = result.rect.y;
				autoL.width = result.rect.width;
				autoL.height = result.rect.height;
				await updateFrame(autoL.id, {
					x: result.rect.x,
					y: result.rect.y,
					width: result.rect.width,
					height: result.rect.height
				});
			}
		}

		if (autoP) {
			const result = computeAutoFrame(globalPoints, 9 / 16, imgAspect);
			if (result) {
				autoP.x = result.rect.x;
				autoP.y = result.rect.y;
				autoP.width = result.rect.width;
				autoP.height = result.rect.height;
				await updateFrame(autoP.id, {
					x: result.rect.x,
					y: result.rect.y,
					width: result.rect.width,
					height: result.rect.height
				});
			}
		}
	}

	// Update composition labels for all frames
	const newLabels = new Map<number, string>();
	for (const frame of frames) {
		// For each frame, compute which focal points are relevant
		const relevantPoints = isAutoFrame(frame.label)
			? globalPoints
			: focalPoints.filter((fp) => !fp.frame_id || fp.frame_id === frame.id).map((fp) => ({ x: fp.x, y: fp.y }));
		if (relevantPoints.length > 0) {
			const pxW = frame.width * imageWidth;
			const pxH = frame.height * imageHeight;
			const frameAspect = pxW / pxH;
			const label = computeFrameLabel(
				{ x: frame.x, y: frame.y, width: frame.width, height: frame.height },
				relevantPoints,
				frameAspect,
				imgAspect
			);
			if (label) newLabels.set(frame.id, label);
		}
	}
	compositionLabels = newLabels;
}

async function handleExport() {
	if (!photo || frames.length === 0) return;
	exporting = true;
	exportProgress = 0;
	exportTotal = frames.length + 1;
	await yieldFrame();
	try {
		await exportFrames(photo, frames, exportDir, exportFormat, 0.92, (current: number, _total: number) => {
			exportProgress = current;
		});
		exportProgress = frames.length;
		await yieldFrame();
		const basename = photo.title?.replace(/\.[^/.]+$/, '').replace(/\s+/g, '-') || 'photo';
		await exportMetadata(photo, frames, focalPoints, `${exportDir}/${basename}.metadata.json`);
		exportProgress = exportTotal;
		toast.success(`Exported ${frames.length} frame${frames.length === 1 ? '' : 's'}`);
	} catch (error) {
		console.error('Export failed:', error);
		toast.error('Export failed');
	} finally {
		exporting = false;
	}
}

async function handleSetOutputDir() {
	const dir = await pickDirectory();
	if (dir) {
		exportDir = dir;
	}
}
</script>

<svelte:window onkeydown={handleKeyDown} onpointermove={handlePointerMove} />

<div class="editor" data-tauri-drag-region>
	<CursorProgress
		progress={busyProgress}
		visible={isBusy}
		x={progressX}
		y={progressY}
		color={busyColor}
	/>

	<div class="stoplight-area">
		<div class="stoplight-shadow"></div>
		<a href="/" class="back-btn" aria-label="Back">
			<svg class="back-chevron" width="14" height="14" viewBox="0 0 20 20" fill="none">
				<path d="M12 15L7 10L12 5" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
			<span class="back-label">Back</span>
		</a>
	</div>

	{#if loading}
		<div class="loading">Loading...</div>
	{:else if photo}
		<div class="editor-body">
			<div class="canvas-area">
				<ImageCanvas
					{photo}
					bind:frames
					bind:focalPoints
					{selectedFrameId}
					{selectedPointId}
					{compositionLabels}
					{activeGuide}
					onchange={handleChange}
					ondeleteframe={handleDeleteFrame}
					onselectframe={(id) => { selectedFrameId = id; selectedPointId = null; }}
					onselectpoint={(id) => { selectedPointId = id; selectedFrameId = null; }}
					onimageload={(w, h) => { imageWidth = w; imageHeight = h; }}
					oncanvasclick={handleCanvasClick}
				/>
			</div>

			{#if previewOpen}
				<PreviewPanel
					{photo}
					{frames}
					{selectedFrameId}
					mode={previewMode}
					{imageWidth}
					{imageHeight}
					onselectframe={(id) => { selectedFrameId = id; selectedPointId = null; }}
					onmodechange={(m) => previewMode = m}
					onclose={() => previewOpen = false}
				/>
			{/if}
		</div>

		<div class="floating-ui">
			<FloatingToolbar
				canUndo={historyIndex > 0}
				canRedo={historyIndex < history.length - 1}
				{saveStatus}
				{panelExpanded}
				{previewOpen}
				onundo={undo}
				onredo={redo}
				onaddframe={handleAddFrame}
				onaddpoint={handleAddFocalPoint}
				onsave={saveMetadata}
				ontoggle={() => panelExpanded = !panelExpanded}
				onpreview={() => previewOpen = !previewOpen}
			/>
			<FramePointPanel
				{frames}
				{focalPoints}
				{imageWidth}
				{imageHeight}
				{selectedFrameId}
				{selectedPointId}
				{compositionLabels}
				expanded={panelExpanded}
				onselectframe={(id) => { selectedFrameId = id; selectedPointId = null; }}
				onselectpoint={(id) => { selectedPointId = id; selectedFrameId = null; }}
				onchange={handleChange}
			/>
		</div>
	{:else}
		<div class="loading">Photo not found</div>
	{/if}

	<CommandBar
		onaddframe={handleAddFrame}
		onaddpoint={handleAddFocalPoint}
		onsave={saveMetadata}
		onexport={handleExport}
		onsetoutputdir={handleSetOutputDir}
		onnextframe={selectNextFrame}
		onprevframe={selectPrevFrame}
		ondeleteframe={() => { if (selectedFrameId != null) handleDeleteFrame(selectedFrameId); }}
		onsetformat={setExportFormat}
		onundo={undo}
		onredo={redo}
		ontoggle={() => panelExpanded = !panelExpanded}
		ondeselect={handleDeselect}
		onpreview={() => previewOpen = !previewOpen}
	/>
</div>

<style>
	.editor {
		width: 100vw;
		height: 100vh;
		overflow: hidden;
		background: #1a1a1a;
	}

	.stoplight-area {
		position: fixed;
		top: 0;
		left: 0;
		z-index: 50;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
	}

	.stoplight-shadow {
		width: 86px;
		height: 40px;
		background: radial-gradient(ellipse at 40px 16px, rgba(0, 0, 0, 0.4) 0%, transparent 70%);
		pointer-events: none;
	}

	.back-btn {
		display: flex;
		align-items: center;
		gap: 0;
		width: 26px;
		height: 26px;
		margin-left: 10px;
		padding: 0 6px;
		color: rgba(255, 255, 255, 0.85);
		text-decoration: none;
		font-size: 12px;
		font-weight: 500;
		background: rgba(0, 0, 0, 0.45);
		backdrop-filter: blur(8px);
		border-radius: 999px;
		overflow: hidden;
		transition: width 0.2s ease, color 0.15s, background 0.15s;
	}

	.back-btn:hover {
		width: 72px;
		color: white;
		background: rgba(0, 0, 0, 0.6);
	}

	.back-chevron {
		flex-shrink: 0;
	}

	.back-label {
		opacity: 0;
		margin-left: 3px;
		white-space: nowrap;
		transition: opacity 0.15s 0.05s;
	}

	.back-btn:hover .back-label {
		opacity: 1;
	}

	.editor-body {
		display: flex;
		width: 100%;
		height: 100%;
	}

	.canvas-area {
		flex: 1;
		min-width: 0;
		height: 100%;
	}

	.loading {
		width: 100%;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		color: #6b7280;
	}

	.floating-ui {
		position: fixed;
		top: 1rem;
		right: 1rem;
		z-index: 50;
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 8px;
	}
</style>
