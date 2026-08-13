<script lang="ts">
import { onMount } from 'svelte';
import { toast } from 'svelte-sonner';
import CommandBar from '$lib/components/CommandBar.svelte';
import CursorProgress from '$lib/components/CursorProgress.svelte';
import FloatingToolbar from '$lib/components/FloatingToolbar.svelte';
import FramePointPanel from '$lib/components/FramePointPanel.svelte';
import GenerateVersionsDialog from '$lib/components/GenerateVersionsDialog.svelte';
import ImageCanvas from '$lib/components/ImageCanvas.svelte';
import PreviewPanel from '$lib/components/PreviewPanel.svelte';
import { REQUIRED_DESTINATIONS } from '$lib/destinations';
import { type FramerStore, FramerSession } from './session.svelte';
import type { CropDoc, ExportFormat, Frame } from './types';

type Report = (done: number, total: number) => void;

interface Props {
	doc: CropDoc;
	imageSrc: string;
	store?: FramerStore;
	historySeqs?: number[];
	historyPos?: number;
	onchange?: (doc: CropDoc) => void;
	onsave?: (doc: CropDoc, report: Report) => Promise<void>;
	onexport?: (frames: Frame[], doc: CropDoc, report: Report) => Promise<void>;
	onsetoutputdir?: () => void;
	onsetformat?: (format: ExportFormat) => void;
}

let { doc, imageSrc, store, historySeqs, historyPos, onchange, onsave, onexport, onsetoutputdir, onsetformat }: Props =
	$props();

// svelte-ignore state_referenced_locally
const session = new FramerSession({
	doc,
	store,
	historySeqs,
	historyPos,
	onchange,
	onerror: (message) => toast.error(message)
});

const GUIDE_TYPES = ['off', 'thirds', 'phi', 'spiral', 'triangle', 'diagonal'] as const;
type GuideType = (typeof GUIDE_TYPES)[number];

let panelExpanded = $state(false);
let previewOpen = $state(false);
let previewMode = $state<'single' | 'all'>('all');
let activeGuide = $state<GuideType>('off');
let generateOpen = $state(false);

let saving = $state(false);
let saveProgress = $state(0);
let saveTotal = $state(0);
let exporting = $state(false);
let exportProgress = $state(0);
let exportTotal = $state(0);

let mouseX = $state(0);
let mouseY = $state(0);
let mouseKnown = $state(false);

let saveStatus = $derived<'saved' | 'saving' | 'unsaved'>(saving ? 'saving' : session.dirty ? 'unsaved' : 'saved');
let isBusy = $derived(saving || exporting);
let busyProgress = $derived(
	exporting && exportTotal > 0 ? exportProgress / exportTotal : saving && saveTotal > 0 ? saveProgress / saveTotal : 0
);
let busyColor = $derived(exporting ? '#22c55e' : '#3b82f6');
let progressX = $derived(mouseKnown ? mouseX : typeof window !== 'undefined' ? window.innerWidth / 2 : 0);
let progressY = $derived(mouseKnown ? mouseY : typeof window !== 'undefined' ? window.innerHeight / 2 : 0);

onMount(() => {
	void session.start();
});

function handlePointerMove(event: PointerEvent) {
	mouseX = event.clientX;
	mouseY = event.clientY;
	mouseKnown = true;
}

async function addFrame() {
	await session.addFrame();
	panelExpanded = true;
}

async function generateVersions(slugs: string[]) {
	if (slugs.length === 0) return;
	const result = await session.generateVersions(slugs);
	if (!result) {
		toast.error('Image is still loading');
		return;
	}
	reportGenerated(result.generated, result.clipped);
}

function reportGenerated(generated: number, clipped: number) {
	if (clipped > 0) {
		toast.warning(`${clipped} version${clipped === 1 ? '' : 's'} cannot fit every required point`);
	} else {
		toast.success(`Generated ${generated} version${generated === 1 ? '' : 's'}`);
	}
}

// A click on empty canvas drops a focal point; holding alt regenerates every required version.
async function handleCanvasClick(normX: number, normY: number, altKey: boolean) {
	await session.addPoint(normX, normY);
	if (altKey) {
		await generateVersions(REQUIRED_DESTINATIONS);
		return;
	}
	await session.recomputeAutoFrames();
	session.touch();
}

async function save() {
	saving = true;
	saveProgress = 0;
	saveTotal = 0;
	try {
		await onsave?.(session.toDoc(), (done, total) => {
			saveProgress = done;
			saveTotal = total;
		});
		session.markSaved();
		toast.success('Saved');
	} catch (error) {
		console.error('Failed to save metadata:', error);
		session.markDirty();
		toast.error('Save failed');
	} finally {
		saving = false;
	}
}

async function runExport() {
	const frames = session.framesToExport;
	if (frames.length === 0) return;
	exporting = true;
	exportProgress = 0;
	exportTotal = 0;
	try {
		await onexport?.(frames, session.toDoc(), (done, total) => {
			exportProgress = done;
			exportTotal = total;
		});
	} finally {
		exporting = false;
	}
}

function cycleGuide() {
	const index = GUIDE_TYPES.indexOf(activeGuide);
	activeGuide = GUIDE_TYPES[(index + 1) % GUIDE_TYPES.length];
}

function isInputFocused(): boolean {
	const el = document.activeElement;
	if (!el) return false;
	const tag = el.tagName.toLowerCase();
	return tag === 'input' || tag === 'textarea' || (el as HTMLElement).isContentEditable;
}

function handleKeyDown(event: KeyboardEvent) {
	if ((event.metaKey || event.ctrlKey) && event.key === 'z') {
		event.preventDefault();
		if (event.shiftKey) {
			session.redo();
		} else {
			session.undo();
		}
	} else if ((event.metaKey || event.ctrlKey) && event.key === 's') {
		event.preventDefault();
		save();
	} else if ((event.metaKey || event.ctrlKey) && event.key === 'e') {
		event.preventDefault();
		runExport();
	} else if (event.key === 'Escape') {
		event.preventDefault();
		session.deselect();
	} else if (!isInputFocused() && !generateOpen) {
		if (event.key === ',') {
			event.preventDefault();
			session.selectPrevFrame();
		} else if (event.key === '.') {
			event.preventDefault();
			session.selectNextFrame();
		} else if (event.key === 'Backspace' || event.key === 'Delete') {
			if (!event.metaKey && !event.ctrlKey) {
				if (session.selectedFrameId != null) {
					event.preventDefault();
					session.deleteFrame(session.selectedFrameId);
				} else if (session.selectedPointId != null) {
					event.preventDefault();
					session.deletePoint(session.selectedPointId);
				}
			}
		} else if (event.key === 'f') {
			event.preventDefault();
			addFrame();
		} else if (event.key === 'p') {
			event.preventDefault();
			session.addPoint();
		} else if (event.key === 'o') {
			event.preventDefault();
			onsetoutputdir?.();
		} else if (event.key === 't') {
			event.preventDefault();
			panelExpanded = !panelExpanded;
		} else if (event.key === 'v') {
			event.preventDefault();
			previewOpen = !previewOpen;
		} else if (event.key === 'a') {
			event.preventDefault();
			generateOpen = true;
		} else if (event.key === 'g') {
			event.preventDefault();
			cycleGuide();
		}
	}
}
</script>

<svelte:window onkeydown={handleKeyDown} onpointermove={handlePointerMove} />

<CursorProgress progress={busyProgress} visible={isBusy} x={progressX} y={progressY} color={busyColor} />

<div class="editor-body">
	<div class="canvas-area">
		<ImageCanvas
			{imageSrc}
			imageName={session.imageName}
			frames={session.frames}
			focalPoints={session.focalPoints}
			selectedFrameId={session.selectedFrameId}
			selectedPointId={session.selectedPointId}
			exportSelection={session.exportSelection}
			compositionLabels={session.compositionLabels}
			{activeGuide}
			onchange={() => session.touch()}
			onclaimframe={(id, geometry) => session.claimFrame(id, geometry)}
			onmovepoint={(id, position) => session.movePoint(id, position)}
			ondeleteframe={(id) => session.deleteFrame(id)}
			onselectframe={(id) => session.selectFrame(id)}
			onselectpoint={(id) => session.selectPoint(id)}
			ontoggleexport={(id) => session.toggleExportSelection(id)}
			onimageload={(w, h) => session.setImageSize(w, h)}
			oncanvasclick={handleCanvasClick}
		/>
	</div>

	{#if previewOpen}
		<PreviewPanel
			{imageSrc}
			frames={session.frames}
			selectedFrameId={session.selectedFrameId}
			exportSelection={session.exportSelection}
			mode={previewMode}
			imageWidth={session.imageWidth}
			imageHeight={session.imageHeight}
			onselectframe={(id) => session.selectFrame(id)}
			ontoggleexport={(id) => session.toggleExportSelection(id)}
			onmodechange={(m) => (previewMode = m)}
			onclose={() => (previewOpen = false)}
		/>
	{/if}
</div>

<div class="floating-ui">
	<FloatingToolbar
		canUndo={session.canUndo}
		canRedo={session.canRedo}
		{saveStatus}
		{panelExpanded}
		{previewOpen}
		exportCount={session.framesToExport.length}
		onundo={() => session.undo()}
		onredo={() => session.redo()}
		onaddframe={addFrame}
		onaddpoint={() => session.addPoint()}
		ongenerate={() => (generateOpen = true)}
		onsave={save}
		onexport={runExport}
		ontoggle={() => (panelExpanded = !panelExpanded)}
		onpreview={() => (previewOpen = !previewOpen)}
	/>
	<FramePointPanel
		frames={session.frames}
		focalPoints={session.focalPoints}
		imageWidth={session.imageWidth}
		imageHeight={session.imageHeight}
		selectedFrameId={session.selectedFrameId}
		selectedPointId={session.selectedPointId}
		exportSelection={session.exportSelection}
		compositionLabels={session.compositionLabels}
		expanded={panelExpanded}
		onselectframe={(id) => session.selectFrame(id)}
		onselectpoint={(id) => session.selectPoint(id)}
		ontoggleexport={(id) => session.toggleExportSelection(id)}
		onclaim={(id, geometry) => session.claimFrame(id, geometry)}
		onlabel={(id, label) => session.setFrameLabel(id, label)}
		onchange={() => session.touch()}
	/>
</div>

<GenerateVersionsDialog open={generateOpen} onclose={() => (generateOpen = false)} ongenerate={generateVersions} />

<CommandBar
	onaddframe={addFrame}
	onaddpoint={() => session.addPoint()}
	ongenerate={() => (generateOpen = true)}
	onsave={save}
	onexport={runExport}
	onsetoutputdir={() => onsetoutputdir?.()}
	onnextframe={() => session.selectNextFrame()}
	onprevframe={() => session.selectPrevFrame()}
	ondeleteframe={() => {
		if (session.selectedFrameId != null) session.deleteFrame(session.selectedFrameId);
	}}
	onsetformat={(format) => onsetformat?.(format)}
	onundo={() => session.undo()}
	onredo={() => session.redo()}
	ontoggle={() => (panelExpanded = !panelExpanded)}
	ondeselect={() => session.deselect()}
	onpreview={() => (previewOpen = !previewOpen)}
	oncycleguide={cycleGuide}
	onselectallexport={() => session.selectAllForExport()}
	onclearexport={() => session.clearExportSelection()}
/>

<style>
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
