<script lang="ts">
import { onMount } from 'svelte';
import { toast } from 'svelte-sonner';
import { detectFacesInImage, invoke } from '$lib/bridge';
import CommandBar from '$lib/components/CommandBar.svelte';
import CursorProgress from '$lib/components/CursorProgress.svelte';
import FloatingToolbar from '$lib/components/FloatingToolbar.svelte';
import FramePointPanel from '$lib/components/FramePointPanel.svelte';
import GenerateVersionsDialog from '$lib/components/GenerateVersionsDialog.svelte';
import ImageCanvas from '$lib/components/ImageCanvas.svelte';
import PreviewPanel from '$lib/components/PreviewPanel.svelte';
import { REQUIRED_DESTINATIONS } from '$lib/destinations';
import { FramerSession, type FramerStore } from './session.svelte';
import type { CropDoc, ExportFormat, Frame, SelectMode } from './types';

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
	oncarve?: () => void;
	imagePath?: string;
}

let {
	doc,
	imageSrc,
	store,
	historySeqs,
	historyPos,
	onchange,
	onsave,
	onexport,
	onsetoutputdir,
	onsetformat,
	oncarve,
	imagePath
}: Props = $props();

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

const PREVIEW_LAYOUT_KEY = 'enframe.preview.layout';

function loadPreviewLayout(): 'sidebar' | 'main' {
	if (typeof localStorage === 'undefined') return 'sidebar';
	return localStorage.getItem(PREVIEW_LAYOUT_KEY) === 'main' ? 'main' : 'sidebar';
}

let panelExpanded = $state(false);
let previewOpen = $state(false);
let previewDismissed = $state(false);
let previewLayout = $state<'sidebar' | 'main'>(loadPreviewLayout());
let activeGuide = $state<GuideType>('off');
let generateOpen = $state(false);

let saving = $state(false);
let saveProgress = $state(0);
let saveTotal = $state(0);
let exporting = $state(false);
let detectingFaces = $state(false);
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

// ponytail: selection reveals the preview until the user closes it once; one flag, no per-frame memory.
// Only the sidebar auto-opens: the main layout covers the canvas, so it never opens itself.
function selectFrame(id: number | null, mode: SelectMode = 'replace') {
	session.selectFrame(id, mode);
	if (id != null && !previewDismissed && previewLayout === 'sidebar') previewOpen = true;
}

async function deleteSelection() {
	const count = await session.deleteSelectedFrames();
	if (count > 0) toast.success(`Deleted ${count} frame${count === 1 ? '' : 's'}`);
}

async function deletePointSelection() {
	const count = await session.deleteSelectedPoints();
	if (count > 0) toast.success(`Deleted ${count} point${count === 1 ? '' : 's'}`);
}

function selectNextFrame() {
	session.selectNextFrame();
	if (!previewDismissed && previewLayout === 'sidebar') previewOpen = true;
}

function selectPrevFrame() {
	session.selectPrevFrame();
	if (!previewDismissed && previewLayout === 'sidebar') previewOpen = true;
}

function togglePreview() {
	previewOpen = !previewOpen;
	previewDismissed = !previewOpen;
}

function closePreview() {
	previewOpen = false;
	previewDismissed = true;
}

function togglePreviewLayout() {
	previewLayout = previewLayout === 'sidebar' ? 'main' : 'sidebar';
	localStorage.setItem(PREVIEW_LAYOUT_KEY, previewLayout);
	if (!previewOpen) {
		previewOpen = true;
		previewDismissed = false;
	}
}

function handlePointerMove(event: PointerEvent) {
	mouseX = event.clientX;
	mouseY = event.clientY;
	mouseKnown = true;
}

async function addFrame() {
	await session.addFrame();
	panelExpanded = true;
}

type DetectedFace = {
	x: number;
	y: number;
	x0: number;
	y0: number;
	x1: number;
	y1: number;
	score: number;
	required: boolean;
};

async function detectFaces() {
	detectingFaces = true;
	try {
		const faces = imagePath
			? await invoke<DetectedFace[]>('detect_faces', { path: imagePath })
			: await detectFacesInImage<DetectedFace[]>(imageSrc);
		const added = await session.addPoints(
			faces
				.filter(
					(face) =>
						!session.focalPoints.some(
							(point) => point.x >= face.x0 && point.x <= face.x1 && point.y >= face.y0 && point.y <= face.y1
						)
				)
				.map((face, index) => ({
					frame_id: undefined,
					x: face.x,
					y: face.y,
					label: `Face ${index + 1}`,
					required: face.required
				}))
		);
		if (added.length === 0) toast.info(faces.length === 0 ? 'No faces found' : 'Faces are already focal points');
		else toast.success(`${added.length} face${added.length === 1 ? '' : 's'} found`);
	} catch (error) {
		console.error('Face detection failed:', error);
		toast.error('Face detection failed');
	} finally {
		detectingFaces = false;
	}
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
	} else if ((event.metaKey || event.ctrlKey) && event.key === 'a' && !isInputFocused()) {
		event.preventDefault();
		session.selectAllFrames();
	} else if (event.key === 'Escape') {
		event.preventDefault();
		session.deselect();
	} else if (!event.metaKey && !event.ctrlKey && !isInputFocused() && !generateOpen) {
		if (event.key === ',') {
			event.preventDefault();
			selectPrevFrame();
		} else if (event.key === '.') {
			event.preventDefault();
			selectNextFrame();
		} else if (event.key === 'Backspace' || event.key === 'Delete') {
			// Points win when one is selected: picking a point no longer clears the frame
			// selection, so that selection alone can no longer say what delete means.
			if (session.pointSelection.size > 0) {
				event.preventDefault();
				deletePointSelection();
			} else if (session.selection.size > 0 || session.selectedFrameId != null) {
				event.preventDefault();
				deleteSelection();
			}
		} else if (event.key === '[') {
			event.preventDefault();
			session.selectPrevPoint();
		} else if (event.key === ']') {
			event.preventDefault();
			session.selectNextPoint();
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
		} else if (event.key === 'v' || event.key === 'V') {
			// Test shiftKey rather than the uppercase letter: Caps Lock also yields 'V'.
			event.preventDefault();
			if (event.shiftKey) togglePreviewLayout();
			else togglePreview();
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
			selection={session.selection}
			pointSelection={session.pointSelection}
			compositionLabels={session.compositionLabels}
			{activeGuide}
			onchange={() => session.touch()}
			onclaimframe={(id, geometry) => session.claimFrame(id, geometry)}
			onmovepoint={(id, position) => session.movePoint(id, position)}
			ondeleteframe={(id) => session.deleteFrame(id)}
			onselectframe={(id) => selectFrame(id)}
			onselectpoint={(id, mode) => session.selectPoint(id, mode)}
			ontoggleselect={(id) => selectFrame(id, 'toggle')}
			onimageload={(w, h) => session.setImageSize(w, h)}
			oncanvasclick={handleCanvasClick}
		/>
		{#if previewOpen && previewLayout === 'main'}
			<div class="preview-main">
				<PreviewPanel
					{imageSrc}
					frames={session.frames}
					selectedFrameId={session.selectedFrameId}
					selection={session.selection}
					imageWidth={session.imageWidth}
					imageHeight={session.imageHeight}
					layout="main"
					onselectframe={(id) => selectFrame(id)}
					ontoggleselect={(id) => selectFrame(id, 'toggle')}
					ontogglelayout={togglePreviewLayout}
					onclose={closePreview}
				/>
			</div>
		{/if}
	</div>

	{#if previewOpen && previewLayout === 'sidebar'}
		<PreviewPanel
			{imageSrc}
			frames={session.frames}
			selectedFrameId={session.selectedFrameId}
			selection={session.selection}
			imageWidth={session.imageWidth}
			imageHeight={session.imageHeight}
			onselectframe={(id) => selectFrame(id)}
			ontoggleselect={(id) => selectFrame(id, 'toggle')}
			ontogglelayout={togglePreviewLayout}
			onclose={closePreview}
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
		ondetectfaces={detectFaces}
		detectingFaces={detectingFaces}
		ongenerate={() => (generateOpen = true)}
		{oncarve}
		onsave={save}
		onexport={runExport}
		ontoggle={() => (panelExpanded = !panelExpanded)}
		onpreview={togglePreview}
	/>
	<FramePointPanel
		frames={session.frames}
		focalPoints={session.focalPoints}
		imageWidth={session.imageWidth}
		imageHeight={session.imageHeight}
		selectedFrameId={session.selectedFrameId}
		selectedPointId={session.selectedPointId}
		selection={session.selection}
		pointSelection={session.pointSelection}
		compositionLabels={session.compositionLabels}
		expanded={panelExpanded}
		onselectframe={(id, mode) => selectFrame(id, mode)}
		onselectpoint={(id, mode) => session.selectPoint(id, mode)}
		ontoggleselect={(id) => selectFrame(id, 'toggle')}
		onselectall={() => session.selectAllFrames()}
		onclearselection={() => session.deselect()}
		ondeleteselection={deleteSelection}
		onclaim={(id, geometry) => session.claimFrame(id, geometry)}
		onlabel={(id, label) => session.setFrameLabel(id, label)}
		onchange={() => session.touch()}
	/>
</div>

<GenerateVersionsDialog open={generateOpen} onclose={() => (generateOpen = false)} ongenerate={generateVersions} />

<CommandBar
	onaddframe={addFrame}
	onaddpoint={() => session.addPoint()}
		ondetectfaces={detectFaces}
	ongenerate={() => (generateOpen = true)}
	onsave={save}
	onexport={runExport}
	onsetoutputdir={() => onsetoutputdir?.()}
	onnextframe={selectNextFrame}
	onprevframe={selectPrevFrame}
	ondeleteframe={deleteSelection}
	onsetformat={(format) => onsetformat?.(format)}
	onundo={() => session.undo()}
	onredo={() => session.redo()}
	ontoggle={() => (panelExpanded = !panelExpanded)}
	ondeselect={() => session.deselect()}
	onpreview={togglePreview}
	onpreviewlayout={togglePreviewLayout}
	oncycleguide={cycleGuide}
	onselectall={() => session.selectAllFrames()}
	onclearselection={() => session.deselect()}
	selectionCount={session.selection.size}
/>

<style>
	.editor-body {
		display: flex;
		width: 100%;
		height: 100%;
	}

	.canvas-area {
		position: relative;
		flex: 1;
		min-width: 0;
		height: 100%;
	}

	.preview-main {
		position: absolute;
		inset: 0;
		z-index: 40;
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
