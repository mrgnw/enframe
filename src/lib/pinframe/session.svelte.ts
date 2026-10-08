// The marking session: the document being edited, the sidecar being drawn, and the mockup's rules for
// marks, skips, the fix list, undo and auto-advance (docs/ui-plan.md, "Marking a lost photo"). Every change
// edits the local photos at once, posts the document, and redraws from the sidecar that comes back.

import { SvelteSet } from 'svelte/reactivity';
import { type Folder, loadFolder, Stale, track } from './api';
import { ghostOffsets, prevLinked } from './trail';
import {
	ADVANCE_MS,
	centre,
	clamp,
	fromSidecar,
	HISTORY_MAX,
	inBounds,
	TOAST_MS,
	TRAIL_DEFAULT,
	TRAIL_MAX,
	TRAIL_MIN,
	ZOOMS,
	type Feature,
	FIRST_FEATURE,
	type Marks,
	type Photo,
	type Point,
	type Sidecar,
	type TrailMode
} from './types';

export type Stage = 'loading' | 'points' | 'tracking' | 'fix' | 'failed';

interface Snapshot {
	marks: Marks;
	photos: Photo[];
	fixList: string[];
	current: number;
}

const copyPhotos = (photos: Photo[]) => photos.map((p) => ({ ...p, pts: p.pts.map((q) => [...q] as Point) }));
const copyMarks = (m: Marks): Marks => JSON.parse(JSON.stringify(m));
const round = (v: number) => Math.round(v * 100) / 100;

export class Session {
	readonly folder: string;
	readonly trailMode: TrailMode;
	stage = $state<Stage>('loading');
	error = $state('');
	photos = $state<Photo[]>([]);
	current = $state(0);
	zoom = $state(1);
	trailN = $state(TRAIL_DEFAULT);
	/** Every photo that has been lost this session; only "Clear fixed" takes marked ones out. */
	fixList = new SvelteSet<string>();
	/** A re-track is running: the filmstrip's dots dim and the status island shows its bar. */
	busy = $state(false);
	toast = $state<{ text: string; canUndo: boolean } | null>(null);
	/** The current photo's points pop in: set for one draw after a mark. */
	just = $state(false);
	/** Set by `go`: the filmstrip scrolls the current thumb into view, centred on a jump. */
	scroll = $state({ serial: 0, jump: false });
	/** The last sidecar's run ID, for anyone comparing results. */
	run = $state('');
	name = FIRST_FEATURE;
	/** This feature's marks, exactly what the CLI gets. */
	marks: Marks = { points: [] };

	private trailLast = TRAIL_DEFAULT;
	/** The zoom was set by the user on this lost photo; otherwise a mark leaves `zoom` out and the CLI chooses. */
	private zoomTouched = false;
	/** The newest `retrack` call's id: only its end clears `busy`. */
	private latestTrack = 0;
	/** Auto-advance landed here before the re-track returned: if this photo links after all, move on again. */
	private autoAdvanced = false;
	private history: Snapshot[] = [];
	private advanceTimer: ReturnType<typeof setTimeout> | undefined;
	private toastTimer: ReturnType<typeof setTimeout> | undefined;

	constructor(folder: string, trailMode: TrailMode) {
		this.folder = folder;
		this.trailMode = trailMode;
	}

	get photo(): Photo {
		return this.photos[this.current];
	}

	get lost(): number {
		return this.photos.filter((p) => p.source === 'lost').length;
	}

	get fixed(): number {
		return [...this.fixList].filter((f) => this.photos.find((p) => p.file === f)?.source !== 'lost').length;
	}

	/** The reference for the current photo: the last one before it where the feature was seen. */
	get ref(): Photo {
		return prevLinked(this.photos, this.current);
	}

	get ghost(): Point[] {
		return this.photo.source === 'lost' ? ghostOffsets(this.photo, this.ref, this.zoom) : [];
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
		this.photos = loaded.photos.map((file) => ({
			file,
			name: file.replace(/\.[^.]+$/, ''),
			w: 0,
			h: 0,
			scale: 1,
			refScale: 1,
			source: 'tracked',
			pts: [],
			feats: null
		}));
		const saved = loaded.features[0];
		if (saved) {
			this.name = saved.name;
			// A photo deleted since the save would make the CLI reject the whole document: drop its entries.
			const here = new Set(loaded.photos);
			const { at, near, skip } = saved.marks;
			const kept = (map: NonNullable<Marks['at']>) =>
				Object.fromEntries(Object.entries(map).filter(([file]) => here.has(file)));
			this.marks = {
				...saved.marks,
				...(at && { at: kept(at) }),
				...(near && { near: kept(near) }),
				...(skip && { skip: skip.filter((file) => here.has(file)) })
			};
			this.stage = 'tracking';
			await this.retrack();
		} else {
			this.stage = 'points';
		}
	}

	/** The first photo's oriented size, from the image itself, before the first track reports sizes. */
	setSize(file: string, w: number, h: number): void {
		const p = this.photos.find((q) => q.file === file);
		if (p && !p.w) {
			p.w = w;
			p.h = h;
		}
	}

	// ---- the Points step: the first photo's reference points, then Track ----

	/** Before tracking, a click adds a point and a click on a point removes it. */
	editStartPoints(pts: Point[]): void {
		this.photos[0].pts = pts;
		this.photos[0].source = 'mark';
	}

	async startTracking(): Promise<void> {
		const first = this.photos[0];
		if (this.stage !== 'points' || !first.pts.length || !first.w) return;
		this.marks = { ...this.marks, points: this.px(first, first.pts) };
		this.stage = 'tracking';
		await this.retrack();
	}

	// ---- the Fix step ----

	/** Normalized points in `p` as the CLI's pixels. */
	private px(p: Photo, pts: Point[]): Point[] {
		return pts.map(([x, y]) => [round(x * p.w), round(y * p.h)]);
	}

	private setMark(p: Photo, pts: Point[], zoom: number | undefined, soft = false): void {
		if (p === this.photos[0]) {
			this.marks.points = this.px(p, pts);
			return;
		}
		const previous = this.marks.at?.[p.file] ?? this.marks.near?.[p.file];
		this.unmark(p);
		const key = soft ? 'near' : 'at';
		this.marks[key] = {
			...this.marks[key],
			[p.file]: {
				points: this.px(p, pts),
				...((zoom ?? previous?.zoom) !== undefined && { zoom: zoom ?? previous?.zoom })
			}
		};
	}

	private unmark(p: Photo): void {
		delete this.marks.at?.[p.file];
		delete this.marks.near?.[p.file];
		this.marks.skip = this.marks.skip?.filter((f) => f !== p.file);
	}

	private remember(): void {
		this.history.push({
			marks: copyMarks(this.marks),
			photos: copyPhotos(this.photos),
			fixList: [...this.fixList],
			current: this.current
		});
		if (this.history.length > HISTORY_MAX) this.history.shift();
	}

	/**
	 * A mark shows where it landed, with an undo, then moves on to the next lost photo. A soft mark places
	 * the photo the same way, but tracking carries on from the photo before it.
	 */
	mark(pts: Point[], soft = false): void {
		const p = this.photo;
		const i = this.current;
		if (p.source !== 'lost') return;
		if (!inBounds(pts)) {
			this.showToast(`Part of the mark falls outside ${p.name}: not marked`, false);
			return;
		}
		this.remember();
		Object.assign(p, { source: soft ? 'soft' : 'mark', pts, scale: this.zoom, feats: null });
		this.setMark(p, pts, this.zoomTouched ? this.zoom : undefined, soft);
		this.pop();
		this.showToast(`${soft ? 'Soft-marked' : 'Marked'} ${p.name}`);
		void this.retrack();
		this.advanceTimer = setTimeout(() => {
			if (this.current === i) this.advance();
		}, ADVANCE_MS);
	}

	/** The feature is hidden here: the photo stays at the estimate and counts as handled, like a mark. */
	skip(): void {
		const p = this.photo;
		const i = this.current;
		if (p.source !== 'lost') return;
		this.remember();
		p.source = 'skipped';
		this.unmark(p);
		this.marks.skip = [...(this.marks.skip ?? []), p.file];
		this.showToast(`Skipped ${p.name}`);
		void this.retrack();
		this.advanceTimer = setTimeout(() => {
			if (this.current === i) this.advance();
		}, ADVANCE_MS);
	}

	/** On a marked photo a click adds a point, and a click on a point removes it. The stage stays put. */
	editPoints(pts: Point[], text: string): void {
		const p = this.photo;
		if (!(p.source === 'mark' || p.source === 'soft') || !pts.length || !inBounds(pts)) return;
		clearTimeout(this.advanceTimer);
		this.remember();
		p.pts = pts;
		this.setMark(p, pts, undefined, p.source === 'soft');
		this.showToast(`${text} ${p.name}`);
		void this.retrack();
	}

	clearMark(): void {
		const p = this.photo;
		if (p.source === 'tracked' || p.source === 'lost' || this.current === 0) return;
		clearTimeout(this.advanceTimer);
		this.remember();
		const done = { mark: 'Cleared mark on', soft: 'Cleared soft mark on', skipped: 'Unskipped' }[p.source];
		this.showToast(`${done} ${p.name}`);
		Object.assign(p, { source: 'lost', feats: p.feats ?? 0 });
		this.unmark(p);
		this.fixList.add(p.file);
		// Lost again: the zoom starts over from the reference, as when landing on a lost photo.
		this.zoom = this.ref.refScale;
		this.zoomTouched = false;
		void this.retrack();
	}

	/** The photo the first track's refusal named, if the document still has one: Clear/Skip need it by name. */
	get failedPhoto(): string | null {
		if (!this.error.includes(' is outside the ')) return null;
		const named = (file: string) => ` ${this.error}`.includes(` ${file}: point `);
		return this.photos.find((p) => named(p.file))?.file ?? null;
	}

	/** A bad mark planted by hand: drop it and try again, or, for the first photo, redo its points. */
	clearBadMark(file: string): void {
		if (file === this.photos[0]?.file) {
			this.marks = { ...this.marks, points: [] };
			Object.assign(this.photos[0], { source: 'tracked', pts: [] });
			this.current = 0;
			this.stage = 'points';
			this.error = '';
			return;
		}
		delete this.marks.at?.[file];
		delete this.marks.near?.[file];
		this.marks.skip = this.marks.skip?.filter((f) => f !== file);
		this.stage = 'tracking';
		this.error = '';
		void this.retrack();
	}

	/** A bad mark on a later photo: move it to `skip` instead of clearing it, and try again. */
	skipBadMark(file: string): void {
		if (file === this.photos[0]?.file) return;
		delete this.marks.at?.[file];
		delete this.marks.near?.[file];
		this.marks.skip = [...(this.marks.skip ?? []), file];
		this.stage = 'tracking';
		this.error = '';
		void this.retrack();
	}

	/** The failed photo's points that lie inside it, by the size the refusal names: what a soft mark keeps. */
	get failedInside(): Point[] {
		const file = this.failedPhoto;
		const size = / outside the (\d+)x(\d+) photo/.exec(this.error);
		const mark = file ? (this.marks.at?.[file] ?? this.marks.near?.[file]) : undefined;
		if (!file || file === this.photos[0]?.file || !size || !mark) return [];
		const [w, h] = [Number(size[1]), Number(size[2])];
		return mark.points.filter(([x, y]) => x >= 0 && x < w && y >= 0 && y < h);
	}

	/** A bad mark on a later photo: keep its points inside the photo as a soft mark, and try again. */
	softBadMark(file: string): void {
		const points = this.failedInside;
		const zoom = (this.marks.at?.[file] ?? this.marks.near?.[file])?.zoom;
		if (!points.length) return;
		delete this.marks.at?.[file];
		this.marks.near = { ...this.marks.near, [file]: { points, ...(zoom !== undefined && { zoom }) } };
		this.stage = 'tracking';
		this.error = '';
		void this.retrack();
	}

	clearFixed(): void {
		const fixed = [...this.fixList].filter((f) => this.photos.find((p) => p.file === f)?.source !== 'lost');
		if (!fixed.length) return;
		this.remember();
		for (const f of fixed) this.fixList.delete(f);
		this.showToast(`Took ${fixed.length} fixed off the list`);
	}

	undo(): void {
		const h = this.history.pop();
		if (!h) return;
		this.marks = h.marks;
		this.photos = h.photos;
		this.fixList.clear();
		for (const f of h.fixList) this.fixList.add(f);
		void this.retrack();
		this.go(h.current, true);
		this.showToast(`Undone on ${this.photos[h.current].name}`, this.history.length > 0);
	}

	/** Posts the document; the newest post wins, and the sidecar that comes back replaces every photo. */
	async retrack(): Promise<void> {
		const mine = ++this.latestTrack;
		this.busy = true;
		try {
			const document: Feature[] = [{ name: this.name, marks: this.marks }];
			this.apply(await track(this.folder, document, this.name));
		} catch (e) {
			if (!(e instanceof Stale)) {
				this.showToast(`Tracking failed: ${(e as Error).message}`, false);
				// The first track failed: the marks are saved, so show why instead of asking for them again.
				if (this.stage === 'tracking') {
					this.stage = 'failed';
					this.error = (e as Error).message;
				}
			}
		}
		if (mine === this.latestTrack) this.busy = false;
	}

	private apply(sidecar: Sidecar): void {
		const sizes = new Map(this.photos.map((p) => [p.file, [p.w, p.h]]));
		this.run = sidecar.run;
		this.photos = sidecar.photos.map((row) => {
			const p = fromSidecar(row);
			const [w, h] = sizes.get(p.file) ?? [0, 0];
			if (!p.w) [p.w, p.h] = [w, h];
			return p;
		});
		for (const p of this.photos) if (p.source === 'lost') this.fixList.add(p.file);
		if (this.stage === 'tracking') {
			this.stage = 'fix';
			const first = this.photos.findIndex((p) => p.source === 'lost');
			this.go(first < 0 ? 0 : first, true);
		} else {
			this.current = clamp(this.current, 0, this.photos.length - 1);
			// A slow re-track: auto-advance moved here while this photo was still lost, and the mark before it
			// linked it after all, so there is nothing to do here. On to the next lost photo.
			if (this.autoAdvanced && this.photo.source === 'tracked' && this.lost) this.advance();
		}
	}

	// ---- moving around ----

	/** After a mark or skip: on to the next lost photo, remembering it was not the user's choice. */
	private advance(): void {
		this.nextInList(1, true);
		this.autoAdvanced = true;
	}

	/** Jumps (Tab, Home, End, undo) centre the current thumb smoothly; steps only keep it in view. */
	go(i: number, jump = false): void {
		clearTimeout(this.advanceTimer);
		this.autoAdvanced = false;
		this.current = clamp(i, 0, this.photos.length - 1);
		if (this.photo.source === 'lost') {
			this.zoom = this.ref.refScale;
			this.zoomTouched = false;
		}
		this.scroll = { serial: this.scroll.serial + 1, jump };
	}

	/** Tab visits the whole fix list, marked photos included; auto-advance skips to the next lost one. */
	nextInList(dir = 1, lostOnly = false): void {
		const n = this.photos.length;
		for (let k = 1; k <= n; k++) {
			const i = (this.current + k * dir + n * 2) % n;
			const p = this.photos[i];
			if (this.fixList.has(p.file) && (!lostOnly || p.source === 'lost')) {
				this.go(i, true);
				return;
			}
		}
	}

	setTrail(n: number): void {
		this.trailN = n;
		if (n) this.trailLast = n;
	}

	trailLess(): void {
		this.setTrail(this.trailN <= TRAIL_MIN ? 0 : this.trailN - 1);
	}

	trailMore(): void {
		this.setTrail(this.trailN ? Math.min(TRAIL_MAX, this.trailN + 1) : TRAIL_MIN);
	}

	toggleTrail(): void {
		this.setTrail(this.trailN ? 0 : this.trailLast);
	}

	setZoom(z: number): void {
		if (this.photo.source !== 'lost') return;
		this.zoom = z;
		this.zoomTouched = true;
	}

	stepZoom(dir: number): void {
		const i = ZOOMS.findIndex((z) => z >= this.zoom - 1e-6);
		this.setZoom(ZOOMS[clamp((i < 0 ? ZOOMS.length - 1 : i) + dir, 0, ZOOMS.length - 1)]);
	}

	/** Where the trail's tag sits: above the topmost point, at the points' centre. */
	get tagAt(): { left: string; top: string } {
		if (!this.photo.pts.length) return { left: '0', top: '0' };
		const [cx] = centre(this.photo);
		return { left: `${cx * 100}%`, top: `${Math.min(...this.photo.pts.map((q) => q[1])) * 100}%` };
	}

	private pop(): void {
		this.just = true;
		setTimeout(() => (this.just = false), 400);
	}

	showToast(text: string, canUndo = true): void {
		clearTimeout(this.toastTimer);
		this.toast = { text, canUndo };
		this.toastTimer = setTimeout(() => (this.toast = null), TOAST_MS);
	}
}
