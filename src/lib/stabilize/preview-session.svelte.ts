// The preview session: the folder's saved document, the sidecar for the chosen pad (the marking screen's
// own when it hands one over), the frames drawn from it, and the export. Pad colour and fps change the
// drawing at once; switching pad between none and a colour changes the crop rects, so that re-tracks (the
// old frames stay up until the new ones are drawn). Each feature has its own destination and export; picking
// one re-tracks it, and "Export all" runs them one after another, since the server runs one per folder.

import { canReveal, exportClip, type Folder, lastSidecar, loadFolder, photoUrl, Stale, trackPad } from './api';
import {
	baseName,
	clipSize,
	fillOf,
	formatBlocked,
	FPS_DEFAULT,
	padArg,
	placeFrame,
	previewSize,
	stripExt,
	type Format,
	type PadChoice
} from './clip';
import type { Feature, SidecarPhoto } from './types';

export type Stage = 'loading' | 'empty' | 'ready' | 'failed';

/** What the screen draws from: one sidecar's rows, the clip's size and the preview frame's. */
export interface View {
	rows: SidecarPhoto[];
	clip: { w: number; h: number };
	size: { w: number; h: number };
}

export interface Exporting {
	status: 'idle' | 'running' | 'done' | 'failed';
	/** 0–1, from the lines the tracker reports. */
	progress: number;
	text: string;
	out: string;
	clip: string;
	seconds: number;
	/** The page is on the instance's machine, so Finder can show the clip. */
	reveal: boolean;
	error: string;
}

const idle = (): Exporting => ({
	status: 'idle',
	progress: 0,
	text: '',
	out: '',
	clip: '',
	seconds: 0,
	reveal: false,
	error: ''
});

/** `IMG_4669.avif  2048x1536  anchor 1028,422  scale 1.00  37 features`: one per photo, once it is tracked. */
const PHOTO_LINE = /^\S+\s+\d+x\d+\s+anchor /;
const DECODERS = 4;

export class Preview {
	readonly folder: string;
	stage = $state<Stage>('loading');
	error = $state('');
	/** The folder without a trailing slash, for the default out path. */
	absFolder = $state('');
	view = $state.raw<View | null>(null);
	/** Bumped when the frames change, since the bitmaps themselves are not reactive. */
	version = $state(0);
	/** Frames drawn so far for the sidecar being rendered, and how many there will be. */
	drawn = $state(0);
	failed = $state.raw<Set<number>>(new Set());
	/** A re-track or a redraw is running behind the frames on screen. */
	busy = $state(false);
	/** The sidecar is being asked for; an export would replace that run, so it waits. */
	tracking = $state(false);
	frame = $state(0);
	playing = $state(false);
	fps = $state(FPS_DEFAULT);
	pad = $state<PadChoice>('none');
	custom = $state('#4b8bf5');
	format = $state<Format>('avif');
	frames = $state(false);
	features = $state.raw<Feature[]>([]);
	/** The feature on screen, an index into `features`. */
	selected = $state(0);
	/** Per feature: the out path without its extension; the format adds that. */
	outBases = $state<string[]>([]);
	/** Per feature. */
	exps = $state<Exporting[]>([]);

	bitmaps: (ImageBitmap | null)[] = [];
	/** The feature the marking screen was on, shown first. */
	private readonly wanted: string | null;
	private generation = 0;
	/** Whether the sidecar on screen was tracked with padding, so a pad change knows if it must re-track. */
	private padded = false;

	constructor(folder: string, feature: string | null = null) {
		this.folder = folder;
		this.wanted = feature;
	}

	get feature(): string {
		return this.features[this.selected]?.name ?? '';
	}

	get outBase(): string {
		return this.outBases[this.selected] ?? '';
	}

	set outBase(base: string) {
		this.outBases[this.selected] = base;
	}

	get exp(): Exporting {
		return this.exps[this.selected] ?? idle();
	}

	/** An export is running for some feature: the server runs one per folder, so nothing else may track. */
	get exporting(): boolean {
		return this.exps.some((e) => e.status === 'running');
	}

	get rows(): SidecarPhoto[] {
		return this.view?.rows ?? [];
	}

	get lost(): number {
		return this.rows.filter((r) => r.source === 'lost').length;
	}

	get fill(): string | null {
		return fillOf(this.pad, this.custom);
	}

	get loading(): boolean {
		return this.view !== null && this.drawn < this.view.rows.length;
	}

	/** The clip's path: the field's path with the format's extension. */
	get out(): string {
		return this.outOf(this.selected);
	}

	private outOf(i: number): string {
		return `${stripExt(this.outBases[i].trim())}.${this.format}`;
	}

	get name(): string {
		return baseName(this.folder);
	}

	async load(): Promise<void> {
		let loaded: Folder;
		try {
			loaded = await loadFolder(this.folder);
		} catch (e) {
			this.stage = 'failed';
			this.error = (e as Error).message;
			return;
		}
		this.absFolder = loaded.folder.replace(/(.)\/+$/, '$1');
		if (!loaded.features.length) {
			this.stage = 'empty';
			return;
		}
		const one = loaded.features.length === 1;
		this.features = loaded.features;
		this.outBases = loaded.features.map((f) => (one ? this.absFolder : `${this.absFolder}-${f.name}`));
		this.exps = loaded.features.map(idle);
		this.selected = Math.max(0, loaded.features.findIndex((f) => f.name === this.wanted));
		await this.show();
	}

	/** Draws the feature on screen: from the marking screen's sidecar when it handed one over, else by tracking. */
	private async show(): Promise<void> {
		this.busy = true;
		const handed = this.pad === 'none' ? lastSidecar(this.folder, this.feature) : null;
		if (!handed) return this.retrack();
		this.padded = false;
		await this.render(handed.photos, ++this.generation);
		this.busy = false;
	}

	/** Puts another feature on screen; not while an export runs, since its track would stop that export. */
	async select(i: number): Promise<void> {
		if (i === this.selected || !this.features[i] || this.exporting) return;
		this.selected = i;
		await this.show();
	}

	/** Tracks with the current pad, then draws the frames from the sidecar that comes back. */
	private async retrack(): Promise<void> {
		const mine = ++this.generation;
		this.busy = true;
		this.tracking = true;
		try {
			const sidecar = await trackPad(this.folder, this.features, this.feature, padArg(this.pad, this.custom));
			this.tracking = false;
			if (mine !== this.generation) return;
			this.padded = this.pad !== 'none';
			await this.render(sidecar.photos, mine);
		} catch (e) {
			if (e instanceof Stale || mine !== this.generation) return;
			this.tracking = false;
			if (this.view) {
				this.exps[this.selected] = { ...idle(), status: 'failed', error: (e as Error).message };
			} else {
				this.stage = 'failed';
				this.error = (e as Error).message;
			}
		}
		if (mine === this.generation) this.busy = false;
	}

	/**
	 * Draws every photo at its crop rect into a frame the size of the preview. The first render shows frames
	 * as they come; a later one replaces the frames on screen only when all are drawn.
	 */
	private async render(rows: SidecarPhoto[], mine: number): Promise<void> {
		const clip = clipSize(rows);
		const size = previewSize(clip);
		const next: (ImageBitmap | null)[] = rows.map(() => null);
		const first = this.view === null;
		const failed = new Set<number>();
		this.drawn = 0;
		if (first) {
			this.bitmaps = next;
			this.view = { rows, clip, size };
			this.stage = 'ready';
		}
		let at = 0;
		const worker = async () => {
			while (at < rows.length && mine === this.generation) {
				const i = at++;
				try {
					const resp = await fetch(photoUrl(this.folder, rows[i].file));
					if (!resp.ok) throw new Error(`${rows[i].file}: ${resp.status}`);
					const photo = await createImageBitmap(await resp.blob());
					if (mine === this.generation) next[i] = placeFrame(photo, rows[i], size.w, size.h);
					photo.close();
				} catch {
					failed.add(i);
				}
				if (mine !== this.generation) return;
				this.drawn++;
				if (first) this.version++;
			}
		};
		await Promise.all(Array.from({ length: DECODERS }, worker));
		if (mine !== this.generation) {
			for (const b of next) b?.close();
			return;
		}
		if (!first) {
			for (const b of this.bitmaps) b?.close();
			this.bitmaps = next;
			this.view = { rows, clip, size };
			this.version++;
		}
		this.failed = failed;
		if (first && !matchMedia('(prefers-reduced-motion: reduce)').matches) this.playing = true;
	}

	dispose(): void {
		this.generation++;
		this.playing = false;
		for (const b of this.bitmaps) b?.close();
		this.bitmaps = [];
	}

	// ---- playback ----

	go(i: number): void {
		const n = this.rows.length;
		if (n) this.frame = ((i % n) + n) % n;
	}

	step(by: number): void {
		this.go(this.frame + by);
	}

	/** Stepping or scrubbing by hand stops the loop. */
	seek(i: number): void {
		this.playing = false;
		this.go(i);
	}

	setFps(fps: number): void {
		if (fps > 0 && fps <= 120) this.fps = fps;
	}

	// ---- pad and format ----

	/** The colour only changes the fill; none to a colour (and back) changes the rects, so it re-tracks. */
	async setPad(pad: PadChoice, custom?: string): Promise<void> {
		if (custom !== undefined) this.custom = custom;
		this.pad = pad;
		this.resetExport();
		if (formatBlocked(this.format, pad)) this.format = 'avif';
		if ((pad !== 'none') !== this.padded) await this.retrack();
	}

	setFormat(format: Format): void {
		if (formatBlocked(format, this.pad)) return;
		this.format = format;
		this.resetExport();
	}

	// ---- export ----

	/** Format, pad and frames are shared, so a change clears every feature's result. */
	resetExport(): void {
		if (!this.exporting) this.exps = this.features.map(idle);
	}

	async export(): Promise<void> {
		if (!this.exporting) await this.exportFeature(this.selected);
	}

	/** Every feature's clip, one after another; a failure stops the rest. */
	async exportAll(): Promise<void> {
		if (this.exporting) return;
		this.resetExport();
		for (let i = 0; i < this.features.length; i++) {
			await this.exportFeature(i);
			if (this.exps[i].status !== 'done') return;
		}
	}

	private async exportFeature(i: number): Promise<void> {
		if (!this.view || !this.outBases[i].trim()) return;
		const base = stripExt(this.outBases[i].trim());
		const out = this.outOf(i);
		const started = Date.now();
		const total = this.view.rows.length;
		// The tracker reports its lines in bursts: nothing while it tracks, every photo's line when tracking ends,
		// then the clip's size when encoding starts. So the bar creeps toward the end of the phase it is in.
		let cap = 0.45;
		this.exps[i] = { ...idle(), status: 'running', progress: 0.03, text: `Tracking ${total} photos…` };
		const exp = this.exps[i];
		const creep = setInterval(() => (exp.progress += (cap - exp.progress) * 0.06), 400);
		try {
			const done = await exportClip(
				{
					folder: this.folder,
					features: this.features,
					feature: this.features[i].name,
					out,
					pad: padArg(this.pad, this.custom),
					fps: this.fps,
					...(this.frames && { frames: `${base} frames` })
				},
				(line) => {
					if (PHOTO_LINE.test(line)) {
						exp.progress = Math.max(exp.progress, 0.5);
						exp.text = 'Tracked, starting the encode…';
					} else if (line.startsWith('clip ')) {
						cap = 0.95;
						exp.progress = Math.max(exp.progress, 0.55);
						exp.text = `Encoding ${total} frames at ${line.slice(5)}…`;
					}
				}
			);
			const size = clipSize(done.sidecar.photos);
			this.exps[i] = {
				...idle(),
				status: 'done',
				progress: 1,
				out: done.out,
				clip: `${size.w}×${size.h}`,
				seconds: (Date.now() - started) / 1000,
				reveal: canReveal()
			};
		} catch (e) {
			this.exps[i] = { ...idle(), status: 'failed', error: (e as Error).message };
		} finally {
			clearInterval(creep);
		}
	}
}
