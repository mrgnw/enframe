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
let imageWidth = $state(0);
let imageHeight = $state(0);

let mouseX = $state(0);
let mouseY = $state(0);
let mouseKnown = $state(false);

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
	} else if (event.key === ',') {
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
	saveStatus = 'unsaved';
	captureSnapshot();
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

<div class="editor">
	<CursorProgress
		progress={busyProgress}
		visible={isBusy}
		x={progressX}
		y={progressY}
		color={busyColor}
	/>

	<a href="/" class="back-btn" aria-label="Back">
		<svg width="20" height="20" viewBox="0 0 20 20" fill="none">
			<path d="M12.5 15L7.5 10L12.5 5" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
		</svg>
	</a>

	{#if loading}
		<div class="loading">Loading...</div>
	{:else if photo}
		<ImageCanvas
			{photo}
			bind:frames
			bind:focalPoints
			{selectedFrameId}
			{selectedPointId}
			onchange={handleChange}
			ondeleteframe={handleDeleteFrame}
			onselectframe={(id) => { selectedFrameId = id; selectedPointId = null; }}
			onselectpoint={(id) => { selectedPointId = id; selectedFrameId = null; }}
			onimageload={(w, h) => { imageWidth = w; imageHeight = h; }}
		/>

		<div class="floating-ui">
			<FloatingToolbar
				canUndo={historyIndex > 0}
				canRedo={historyIndex < history.length - 1}
				{saveStatus}
				{panelExpanded}
				onundo={undo}
				onredo={redo}
				onaddframe={handleAddFrame}
				onaddpoint={handleAddFocalPoint}
				onsave={saveMetadata}
				ontoggle={() => panelExpanded = !panelExpanded}
			/>
			<FramePointPanel
				{frames}
				{focalPoints}
				{imageWidth}
				{imageHeight}
				{selectedFrameId}
				{selectedPointId}
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
	/>
</div>

<style>
	.editor {
		width: 100vw;
		height: 100vh;
		overflow: hidden;
		background: #1a1a1a;
	}

	.back-btn {
		position: fixed;
		top: 1rem;
		left: 1rem;
		z-index: 50;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 2.5rem;
		height: 2.5rem;
		border-radius: 0.5rem;
		background: rgba(0, 0, 0, 0.5);
		color: rgba(255, 255, 255, 0.8);
		text-decoration: none;
		backdrop-filter: blur(8px);
		transition: background 0.15s, color 0.15s;
	}

	.back-btn:hover {
		background: rgba(0, 0, 0, 0.7);
		color: white;
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
