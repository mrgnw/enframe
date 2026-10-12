<script lang="ts">
import { onMount } from 'svelte';
import { toast } from 'svelte-sonner';
import { getImageSrc, hasNativeShell, onDragDrop, pickFiles, readTextFile } from '$lib/bridge';
import { useDock } from '$lib/components/dock.svelte';
import {
	AVIF_SPEED,
	cropToBlob,
	EXPORT_QUALITY,
	encodingFor,
	loadImage,
	prepareEncoder
} from '$lib/framer/canvas-export';
import Framer from '$lib/framer/Framer.svelte';
import {
	download,
	fileSystemAccess,
	type PickedFile,
	pickImageFiles,
	pickOutputDirectory,
	saveAs,
	writeInto
} from '$lib/framer/files';
import { MemoryStore } from '$lib/framer/memory-store.svelte';
import { generateFrameFilename, getDefaultExportDir, sidecarJson, sidecarName } from '$lib/framer/output';
import { decodeRaw, isRawName } from '$lib/framer/raw';
import {
	CROP_DOC_VERSION,
	type CropDoc,
	type DocImage,
	docFromSidecar,
	type ExportFormat,
	type Frame
} from '$lib/framer/types';
import { loadRecents, type Recent, recentId, relativeTime, rememberRecent, type StandaloneRecent } from '$lib/recents';
import StartVisual from '$lib/StartVisual.svelte';

type Report = (done: number, total: number) => void;

let { libraryHref = null }: { libraryHref?: string | null } = $props();

useDock({ title });

function yieldFrame(): Promise<void> {
	return new Promise((r) => requestAnimationFrame(() => r()));
}

function isSidecar(name: string): boolean {
	return name.toLowerCase().endsWith('.json');
}

let doc = $state<CropDoc | null>(null);
let store = $state<MemoryStore | null>(null);
let generation = $state(0);
let imageSrc = $state('');
let imageEl: HTMLImageElement | null = null;
let nativePath = $state<string | null>(null);
let mismatch = $state<string | null>(null);
let dragging = $state(false);
let opening = $state(false);
let openingNote = $state('');
let recents = $state<Recent[]>([]);
let pending: CropDoc | null = null;
const openedPage = Date.now();

let format = $state<ExportFormat>('avif');
let exportDir = $state('');
let outputDir = $state<FileSystemDirectoryHandle | null>(null);
let sidecarHandle: FileSystemFileHandle | null = null;

let imageName = $derived(doc?.image.name ?? '');
let destination = $derived(
	nativePath ? exportDir : outputDir ? `${outputDir.name}/` : fileSystemAccess ? 'no folder set' : 'downloads'
);

onMount(() => {
	recents = loadRecents();
	if (!hasNativeShell()) return;
	let unlisten: (() => void) | null = null;
	void onDragDrop((payload) => {
		if (payload.type === 'enter') dragging = true;
		else if (payload.type === 'leave') dragging = false;
		else if (payload.type === 'drop') {
			dragging = false;
			void openNative(payload.paths);
		}
	}).then((off) => {
		unlisten = off;
	});
	return () => unlisten?.();
});

// Opening

function startDoc(image: DocImage, sidecarText: string | null) {
	const opened = sidecarText ? fromSidecar(sidecarText, image) : pending ? fromRecent(pending, image) : blankDoc(image);
	pending = null;
	doc = opened;
	store = new MemoryStore(opened);
	sidecarHandle = null;
	generation += 1;
	remember(opened);
}

function remember(edited: CropDoc) {
	if (!edited.image.name) return;
	recents = rememberRecent({
		kind: 'standalone',
		name: edited.image.name,
		width: edited.image.width,
		height: edited.image.height,
		openedAt: Date.now(),
		doc: edited
	});
}

// The file behind a recent entry cannot be reopened on its own, so the user picks it
// again and the frames land on whatever they picked — with the same warning a stale
// sidecar earns.
async function openRecent(entry: StandaloneRecent) {
	pending = entry.doc;
	toast.info(`Choose ${entry.name} again to bring its frames back`);
	await chooseImage();
	pending = null;
}

function fromRecent(saved: CropDoc, image: DocImage): CropDoc {
	const sized = saved.image.width > 0 && (saved.image.width !== image.width || saved.image.height !== image.height);
	mismatch = sized ? `Those frames were made for a ${saved.image.width}×${saved.image.height} image` : null;
	return { ...saved, version: CROP_DOC_VERSION, image };
}

function blankDoc(image: DocImage): CropDoc {
	return { version: CROP_DOC_VERSION, image, frames: [], focalPoints: [] };
}

// The image on disk is the truth; a sidecar that disagrees was probably written before
// the file was renamed or resized, which is worth a warning but not a refusal. A sidecar
// that will not parse at all still must not cost the user the image they just opened.
function fromSidecar(text: string, image: DocImage): CropDoc {
	let parsed: unknown;
	try {
		parsed = JSON.parse(text);
	} catch {
		mismatch = 'That sidecar is not readable JSON';
		return blankDoc(image);
	}
	const loaded = docFromSidecar(parsed as Parameters<typeof docFromSidecar>[0]);
	const named = loaded.image.name && loaded.image.name !== image.name;
	const sized = loaded.image.width > 0 && (loaded.image.width !== image.width || loaded.image.height !== image.height);
	mismatch = named
		? `Sidecar was written for "${loaded.image.name}"`
		: sized
			? `Sidecar was written for a ${loaded.image.width}×${loaded.image.height} image`
			: null;
	return { ...loaded, image };
}

async function measure(src: string): Promise<DocImage & { element: HTMLImageElement }> {
	const element = await loadImage(src);
	return { name: '', width: element.naturalWidth, height: element.naturalHeight, element };
}

// A browser cannot put a RAW in an <img>, so develop it first and hand the rest of the
// app the decoded pixels. Everything downstream — the natural dimensions the CropDoc
// records, and the canvas the exporter crops — then reads from the decode, not the file.
async function sourceFor(file: File): Promise<string> {
	if (!isRawName(file.name)) return URL.createObjectURL(file);
	openingNote = `Developing ${file.name}…`;
	const pending = toast.loading(openingNote);
	try {
		return URL.createObjectURL(await decodeRaw(file));
	} finally {
		toast.dismiss(pending);
		openingNote = '';
	}
}

async function openPicked(picked: PickedFile[]) {
	if (picked.length === 0) return;
	const image = picked.find((p) => !isSidecar(p.file.name));
	const sidecar = picked.find((p) => isSidecar(p.file.name));

	if (!image) {
		if (!doc || !sidecar) {
			toast.error('Drop an image alongside the sidecar');
			return;
		}
		applySidecarText(await sidecar.file.text());
		return;
	}

	opening = true;
	try {
		const url = await sourceFor(image.file);
		const measured = await measure(url);
		if (imageSrc.startsWith('blob:')) URL.revokeObjectURL(imageSrc);
		imageSrc = url;
		imageEl = measured.element;
		nativePath = null;
		mismatch = null;
		startDoc(
			{ name: image.file.name, width: measured.width, height: measured.height },
			sidecar ? await sidecar.file.text() : null
		);
	} catch (error) {
		console.error('Failed to open image:', error);
		toast.error(error instanceof Error && error.message ? error.message : 'Could not open that image');
	} finally {
		opening = false;
	}
}

async function openNative(paths: string[]) {
	if (paths.length === 0) return;
	const path = paths.find((p) => !isSidecar(p));
	const sidecarPath = paths.find((p) => isSidecar(p));

	if (!path) {
		if (!doc || !sidecarPath) {
			toast.error('Drop an image alongside the sidecar');
			return;
		}
		applySidecarText(await readTextFile(sidecarPath));
		return;
	}

	opening = true;
	try {
		const src = getImageSrc(path);
		const measured = await measure(src);
		imageSrc = src;
		imageEl = measured.element;
		nativePath = path;
		exportDir = getDefaultExportDir(path);
		mismatch = null;
		startDoc(
			{ name: path.split('/').pop() ?? path, width: measured.width, height: measured.height },
			sidecarPath ? await readTextFile(sidecarPath) : null
		);
	} catch (error) {
		console.error('Failed to open image:', error);
		toast.error('Could not open that image');
	} finally {
		opening = false;
	}
}

function applySidecarText(text: string) {
	if (!doc) return;
	try {
		startDoc(doc.image, text);
		toast.success('Loaded frames from the sidecar');
	} catch (error) {
		console.error('Failed to read sidecar:', error);
		toast.error('That sidecar could not be read');
	}
}

async function chooseImage() {
	if (hasNativeShell()) {
		const selected = await pickFiles({ title: 'Open image', multiple: true });
		if (selected) await openNative(selected);
		return;
	}
	await openPicked(await pickImageFiles());
}

function handleDragOver(event: DragEvent) {
	event.preventDefault();
	dragging = true;
}

function handleDragLeave(event: DragEvent) {
	if (event.relatedTarget === null) dragging = false;
}

async function handleDrop(event: DragEvent) {
	event.preventDefault();
	dragging = false;
	if (hasNativeShell()) return;
	const files = Array.from(event.dataTransfer?.files ?? []);
	await openPicked(files.map((file) => ({ file, handle: null })));
}

// Saving and exporting

function sidecarBlob(edited: CropDoc): Blob {
	return new Blob([sidecarJson(edited, nativePath ?? undefined)], { type: 'application/json' });
}

async function save(edited: CropDoc, report: Report) {
	report(0, 1);
	await yieldFrame();
	const name = sidecarName(imageName);

	if (nativePath) {
		const { exportMetadata } = await import('$lib/export');
		await exportMetadata(nativePhoto(edited), edited, `${exportDir}/${name}`);
	} else if (outputDir) {
		await writeInto(outputDir, name, sidecarBlob(edited));
	} else {
		const handle = await saveAs(name, sidecarBlob(edited), sidecarHandle);
		if (fileSystemAccess && !handle) throw new Error('Save cancelled');
		sidecarHandle = handle;
	}
	remember(edited);
	report(1, 1);
}

function nativePhoto(edited: CropDoc) {
	return {
		id: 0,
		file_path: nativePath ?? '',
		title: edited.image.name,
		width: edited.image.width,
		height: edited.image.height,
		created_at: ''
	};
}

async function runExport(frames: Frame[], edited: CropDoc, report: Report) {
	const total = frames.length + 1;
	report(0, total);
	await yieldFrame();
	try {
		if (nativePath) {
			await exportNative(frames, edited, report, total);
		} else {
			await exportInBrowser(frames, edited, report, total);
		}
		report(total, total);
		toast.success(`Exported ${frames.length} frame${frames.length === 1 ? '' : 's'}`);
	} catch (error) {
		console.error('Export failed:', error);
		toast.error(error instanceof Error ? error.message : 'Export failed');
	}
}

// The desktop app keeps the Rust exporter: better encoders, and it reads the RAW
// originals a canvas cannot decode.
async function exportNative(frames: Frame[], edited: CropDoc, report: Report, total: number) {
	const { exportFrames, exportMetadata } = await import('$lib/export');
	const photo = nativePhoto(edited);
	await exportFrames(photo, frames, exportDir, format, EXPORT_QUALITY, AVIF_SPEED, (current: number) =>
		report(current, total)
	);
	report(frames.length, total);
	await yieldFrame();
	await exportMetadata(photo, { ...edited, frames }, `${exportDir}/${sidecarName(imageName)}`);
}

async function exportInBrowser(frames: Frame[], edited: CropDoc, report: Report, total: number) {
	const image = imageEl ?? (await loadImage(imageSrc));
	const encoding = encodingFor(format);
	await prepareEncoder(encoding);

	const used = new Set<string>();
	for (let i = 0; i < frames.length; i++) {
		report(i, total);
		await yieldFrame();
		const name = generateFrameFilename({ title: imageName }, frames[i], i, encoding.extension, used);
		await put(name, await cropToBlob(image, frames[i], encoding));
	}
	report(frames.length, total);
	await yieldFrame();
	await put(sidecarName(imageName), sidecarBlob({ ...edited, frames }));
}

async function put(name: string, blob: Blob) {
	if (outputDir) {
		await writeInto(outputDir, name, blob);
		return;
	}
	download(name, blob);
}

async function setOutputDir() {
	if (nativePath) {
		const { pickDirectory } = await import('$lib/export');
		const dir = await pickDirectory();
		if (dir) exportDir = dir;
		return;
	}
	if (!fileSystemAccess) {
		toast.info('This browser cannot write to a folder — exports will download');
		return;
	}
	const dir = await pickOutputDirectory();
	if (dir) {
		outputDir = dir;
		sidecarHandle = null;
		toast.success(`Exporting to ${dir.name}`);
	}
}

function setFormat(next: ExportFormat) {
	format = next;
	toast.success(`Export format: ${next.toUpperCase()}`);
}
</script>

<svelte:window ondragover={handleDragOver} ondragleave={handleDragLeave} ondrop={handleDrop} />

<!-- After the app switch in the layout's dock: the photo being framed, and the way back to the library. -->
{#snippet title()}
	{#if doc}
		<span class="sep"></span>
		{#if libraryHref}
			<a href={libraryHref} class="btn icon" aria-label="Library" title="Library">
				<svg width="14" height="14" viewBox="0 0 20 20" fill="none" aria-hidden="true">
					<path d="M12 15L7 10L12 5" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</a>
		{/if}
		<span class="name">{imageName}</span>
		<span class="dot">&middot;</span>
		<span class="target">{destination}</span>
		<button class="link" onclick={chooseImage}>Open another</button>
	{/if}
{/snippet}

<div class="standalone" class:dragging data-tauri-drag-region>
	<div class="corner">
		<div class="stoplight-shadow"></div>
	</div>

	{#if doc}
		{#if mismatch}
			<div class="mismatch" role="status">
				{mismatch}. Frames were loaded anyway.
				<button class="link" onclick={() => (mismatch = null)}>Dismiss</button>
			</div>
		{/if}

		{#key generation}
			<Framer
				doc={doc}
				store={store ?? undefined}
				{imageSrc}
				imagePath={nativePath ?? undefined}
				onsave={save}
				onexport={runExport}
				onsetoutputdir={setOutputDir}
				onsetformat={setFormat}
			/>
		{/key}
	{:else}
		<div class="empty">
			<div class="visual"><StartVisual kind="frame" /></div>
			<h1>One photo, <b>many stories</b></h1>
			<p class="lede">
				Drop an image and frame it more than one way. Each crop is its own story, saved next to the photo.
			</p>
			<p class="lede small">
				Bring its <code>.metadata.json</code> along and the crops you made last time come back with it.
			</p>
			<div class="actions">
				<button class="btn primary large" onclick={chooseImage} disabled={opening}>
					{opening ? openingNote || 'Opening…' : 'Choose an image'}
				</button>
				{#if libraryHref}
					<a class="browse" href={libraryHref}>Browse library</a>
				{/if}
			</div>
			<p class="fine">
				Nothing is imported and nothing leaves this machine.
				{#if !fileSystemAccess && !hasNativeShell()}
					Exports arrive in your downloads folder.
				{/if}
			</p>

			{#if recents.length > 0}
				<ul class="recents">
					{#each recents as entry (recentId(entry))}
						<li>
							{#if entry.kind === 'library'}
								<a class="recent" href="/edit/{entry.photoId}">{@render recentRow(entry)}</a>
							{:else}
								<button class="recent" onclick={() => openRecent(entry)}>{@render recentRow(entry)}</button>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}
		</div>
	{/if}
</div>

{#snippet recentRow(entry: Recent)}
	<span class="recent-name">{entry.name || 'Untitled'}</span>
	<span class="recent-dims">{entry.width}&times;{entry.height}</span>
	<span class="recent-when">{relativeTime(entry.openedAt, openedPage)}</span>
{/snippet}

<style>
	.standalone {
		position: relative;
		width: 100vw;
		height: 100vh;
		overflow: hidden;
		background: #1a1a1a;
	}

	.standalone.dragging::after {
		content: '';
		position: absolute;
		inset: 12px;
		border: 2px dashed #2563eb;
		border-radius: 12px;
		pointer-events: none;
		z-index: 60;
	}

	.corner {
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







	.name {
		max-width: 28vw;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}
	.target {
		color: var(--subtle);
	}

	.dot {
		opacity: 0.5;
	}

	.link {
		color: #60a5fa;
		font-size: inherit;
		background: none;
		border: none;
		padding: 0;
		cursor: pointer;
	}

	.link:hover {
		color: #93c5fd;
	}

	.mismatch {
		position: fixed;
		top: 62px;
		left: 14px;
		z-index: 50;
		display: flex;
		gap: 0.5rem;
		padding: 0.35rem 0.7rem;
		color: #fbbf24;
		font-size: 0.72rem;
		background: rgba(60, 40, 0, 0.6);
		border: 1px solid rgba(251, 191, 36, 0.3);
		border-radius: 0.5rem;
	}

	.empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 100%;
		padding: 0 2rem;
		text-align: center;
	}

	.visual {
		margin-bottom: 2.5rem;
		transition: opacity 0.2s;
	}
	.standalone.dragging .visual {
		opacity: 0.4;
	}

	h1 {
		margin: 0 0 0.75rem;
		color: #f4f4f5;
		font-size: 1.65rem;
		font-weight: 600;
		letter-spacing: -0.02em;
		line-height: 1.2;
	}
	h1 b {
		color: var(--brand);
		font-weight: inherit;
	}

	.lede {
		max-width: 42ch;
		margin: 0 0 0.5rem;
		color: #a1a1aa;
		font-size: 0.9rem;
		line-height: 1.65;
	}

	.lede.small {
		margin: 0 0 1.75rem;
		color: #71717a;
		font-size: 0.8rem;
	}

	code {
		padding: 0.1rem 0.3rem;
		color: #d4d4d8;
		font-size: 0.82em;
		background: #27272a;
		border-radius: 0.25rem;
	}



	.actions .btn:disabled {
		opacity: 0.6;
		cursor: progress;
	}

	.actions {
		display: flex;
		align-items: center;
		gap: 1.1rem;
	}

	.browse {
		padding-bottom: 1px;
		color: #71717a;
		font-size: 0.8rem;
		text-decoration: none;
		border-bottom: 1px solid #3f3f46;
		transition: color 0.15s, border-color 0.15s;
	}

	.browse:hover {
		color: #d4d4d8;
		border-color: #71717a;
	}

	.recents {
		width: 100%;
		max-width: 30rem;
		max-height: 28vh;
		margin: 2.25rem 0 0;
		padding: 0.35rem 0 0;
		overflow-y: auto;
		list-style: none;
		border-top: 1px solid #27272a;
	}

	.recent {
		display: grid;
		grid-template-columns: 1fr auto auto;
		align-items: baseline;
		gap: 0.85rem;
		width: 100%;
		padding: 0.35rem 0.5rem;
		color: #a1a1aa;
		font: inherit;
		font-size: 0.78rem;
		text-align: left;
		text-decoration: none;
		background: none;
		border: none;
		border-radius: 0.35rem;
		cursor: pointer;
		transition: background 0.15s, color 0.15s;
	}

	.recent:hover {
		color: #f4f4f5;
		background: #27272a;
	}

	.recent-name {
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}

	.recent-dims,
	.recent-when {
		color: #52525b;
		font-size: 0.72rem;
		font-variant-numeric: tabular-nums;
	}

	.recent-when {
		min-width: 4.5rem;
		text-align: right;
	}

	.fine {
		max-width: 40ch;
		margin: 1.75rem 0 0;
		color: #52525b;
		font-size: 0.72rem;
		line-height: 1.6;
	}

</style>
