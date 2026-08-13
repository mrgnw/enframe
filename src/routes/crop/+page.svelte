<script lang="ts">
import { onMount } from 'svelte';
import { toast } from 'svelte-sonner';
import { getImageSrc, isTauri, onDragDrop, pickFiles, readTextFile } from '$lib/bridge';
import { cropToBlob, encodingFor, loadImage } from '$lib/framer/canvas-export';
import {
	download,
	fileSystemAccess,
	type PickedFile,
	pickImageFiles,
	pickOutputDirectory,
	saveAs,
	writeInto
} from '$lib/framer/files';
import Framer from '$lib/framer/Framer.svelte';
import { MemoryStore } from '$lib/framer/memory-store.svelte';
import { generateFrameFilename, getDefaultExportDir, sidecarJson, sidecarName } from '$lib/framer/output';
import {
	type CropDoc,
	CROP_DOC_VERSION,
	type DocImage,
	docFromSidecar,
	type ExportFormat,
	type Frame
} from '$lib/framer/types';

type Report = (done: number, total: number) => void;

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

let format = $state<ExportFormat>('avif');
let exportDir = $state('');
let outputDir = $state<FileSystemDirectoryHandle | null>(null);
let sidecarHandle: FileSystemFileHandle | null = null;

let imageName = $derived(doc?.image.name ?? '');
let destination = $derived(
	nativePath ? exportDir : outputDir ? `${outputDir.name}/` : fileSystemAccess ? 'no folder set' : 'downloads'
);

onMount(() => {
	if (!isTauri()) return;
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
	const opened = sidecarText ? fromSidecar(sidecarText, image) : blankDoc(image);
	doc = opened;
	store = new MemoryStore(opened);
	sidecarHandle = null;
	generation += 1;
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
		const url = URL.createObjectURL(image.file);
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
		toast.error('Could not open that image');
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
	if (isTauri()) {
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
	if (isTauri()) return;
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
		toast.error('Export failed');
	}
}

// The desktop app keeps the Rust exporter: better encoders, and it reads the RAW
// originals a canvas cannot decode.
async function exportNative(frames: Frame[], edited: CropDoc, report: Report, total: number) {
	const { exportFrames, exportMetadata } = await import('$lib/export');
	const photo = nativePhoto(edited);
	await exportFrames(photo, frames, exportDir, format, 0.92, (current: number) => report(current, total));
	report(frames.length, total);
	await yieldFrame();
	await exportMetadata(photo, { ...edited, frames }, `${exportDir}/${sidecarName(imageName)}`);
}

async function exportInBrowser(frames: Frame[], edited: CropDoc, report: Report, total: number) {
	const image = imageEl ?? (await loadImage(imageSrc));
	const encoding = encodingFor(format);
	if (encoding.substituted) toast.warning('Browsers cannot encode AVIF — exporting WebP instead');

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
	const encoding = encodingFor(next);
	if (!nativePath && encoding.substituted) {
		toast.warning('Browsers cannot encode AVIF — exports will be WebP');
		return;
	}
	toast.success(`Export format: ${next.toUpperCase()}`);
}
</script>

<svelte:window ondragover={handleDragOver} ondragleave={handleDragLeave} ondrop={handleDrop} />

<div class="standalone" class:dragging data-tauri-drag-region>
	<div class="corner">
		<div class="stoplight-shadow"></div>
		<a href="/" class="back-btn" aria-label="Library">
			<svg class="back-chevron" width="14" height="14" viewBox="0 0 20 20" fill="none" aria-hidden="true">
				<path d="M12 15L7 10L12 5" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
			<span class="back-label">Library</span>
		</a>
	</div>

	{#if doc}
		<div class="status">
			<span class="name">{imageName}</span>
			<span class="dot">&middot;</span>
			<span class="target">{destination}</span>
			<button class="link" onclick={chooseImage}>Open another</button>
		</div>

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
				onsave={save}
				onexport={runExport}
				onsetoutputdir={setOutputDir}
				onsetformat={setFormat}
			/>
		{/key}
	{:else}
		<div class="empty">
			<div class="marks" aria-hidden="true">
				<span class="mark tl"></span>
				<span class="mark tr"></span>
				<span class="mark bl"></span>
				<span class="mark br"></span>
				<span class="grain"></span>
			</div>
			<h1>One image, no library</h1>
			<p class="lede">
				Drop a photo here to frame it. Bring its <code>.metadata.json</code> along and the crops you made last time
				come back with it.
			</p>
			<button class="open" onclick={chooseImage} disabled={opening}>
				{opening ? 'Opening…' : 'Choose an image'}
			</button>
			<p class="fine">
				Nothing is imported and nothing leaves this machine.
				{#if !fileSystemAccess && !isTauri()}
					Exports arrive in your downloads folder.
				{/if}
			</p>
		</div>
	{/if}
</div>

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

	.back-btn {
		display: flex;
		align-items: center;
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
		width: 84px;
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

	.status {
		position: fixed;
		top: 14px;
		left: 120px;
		z-index: 50;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		max-width: 46vw;
		padding: 0.3rem 0.7rem;
		color: #9ca3af;
		font-size: 0.72rem;
		background: rgba(0, 0, 0, 0.45);
		backdrop-filter: blur(8px);
		border-radius: 999px;
	}

	.name {
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
		color: #e5e7eb;
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
		top: 52px;
		left: 120px;
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

	.marks {
		position: relative;
		width: 132px;
		height: 92px;
		margin-bottom: 2.5rem;
	}

	.mark {
		position: absolute;
		width: 26px;
		height: 26px;
		border: 2px solid #3f3f46;
		transition: border-color 0.2s;
	}

	.tl { top: 0; left: 0; border-right: none; border-bottom: none; }
	.tr { top: 0; right: 0; border-left: none; border-bottom: none; }
	.bl { bottom: 0; left: 0; border-right: none; border-top: none; }
	.br { bottom: 0; right: 0; border-left: none; border-top: none; }

	.grain {
		position: absolute;
		inset: 50% auto auto 50%;
		width: 7px;
		height: 7px;
		margin: -3.5px 0 0 -3.5px;
		background: #2563eb;
		border-radius: 999px;
		animation: drift 5s ease-in-out infinite;
	}

	.standalone.dragging .mark {
		border-color: #2563eb;
	}

	@keyframes drift {
		0%, 100% { transform: translate(-22px, -14px); }
		50% { transform: translate(24px, 15px); }
	}

	h1 {
		margin: 0 0 0.75rem;
		color: #f4f4f5;
		font-size: 1.65rem;
		font-weight: 600;
		letter-spacing: -0.02em;
	}

	.lede {
		max-width: 42ch;
		margin: 0 0 1.75rem;
		color: #a1a1aa;
		font-size: 0.9rem;
		line-height: 1.65;
	}

	code {
		padding: 0.1rem 0.3rem;
		color: #d4d4d8;
		font-size: 0.82em;
		background: #27272a;
		border-radius: 0.25rem;
	}

	.open {
		padding: 0.55rem 1.15rem;
		color: white;
		font-size: 0.85rem;
		background: #2563eb;
		border: none;
		border-radius: 0.5rem;
		cursor: pointer;
		transition: background 0.15s, transform 0.15s;
	}

	.open:hover:not(:disabled) {
		background: #3b82f6;
		transform: translateY(-1px);
	}

	.open:disabled {
		opacity: 0.6;
		cursor: progress;
	}

	.fine {
		max-width: 40ch;
		margin: 1.75rem 0 0;
		color: #52525b;
		font-size: 0.72rem;
		line-height: 1.6;
	}

	@media (prefers-reduced-motion: reduce) {
		.grain { animation: none; }
		.open:hover:not(:disabled) { transform: none; }
	}
</style>
