import { computeAutoFrame, computeFrameLabel, legacyAutoAspect } from '../composition';
import { findDestination } from '../destinations';
import {
	clampPos,
	HISTORY_DEBOUNCE_MS,
	planAppend,
	redoSeq,
	remapId,
	remapSelection,
	serializeSnapshot,
	undoSeq
} from '../history';
import {
	type CropDoc,
	CROP_DOC_VERSION,
	cycleFrameId,
	type FocalPoint,
	type Frame,
	nextLocalId,
	nextPointSelection,
	nextSelection,
	type PointSelectMode,
	type SelectMode
} from './types';

export interface RestoredHistory {
	frames: Frame[];
	focalPoints: FocalPoint[];
	frameIds: Map<number, number>;
}

// Everything the framer needs from a host that keeps documents somewhere.
// ponytail: a host without one edits in memory and cannot undo; the browser build
// supplies its own sidecar-backed store in the next stage.
export interface FramerStore {
	addFrame(frame: Omit<Frame, 'id'>): Promise<number>;
	updateFrame(id: number, patch: Partial<Frame>): Promise<void>;
	deleteFrame(id: number): Promise<void>;
	addPoint(point: Omit<FocalPoint, 'id'>): Promise<number>;
	updatePoint(id: number, patch: Partial<FocalPoint>): Promise<void>;
	deletePoint(id: number): Promise<void>;
	saveHistory(seq: number, drop: number[], snapshot: string): Promise<void>;
	loadHistory(seq: number): Promise<RestoredHistory | null>;
}

export interface SessionOptions {
	doc: CropDoc;
	store?: FramerStore;
	historySeqs?: number[];
	historyPos?: number;
	onchange?: (doc: CropDoc) => void;
	onerror?: (message: string) => void;
}

export interface GenerateResult {
	generated: number;
	clipped: number;
}

export class FramerSession {
	frames = $state<Frame[]>([]);
	focalPoints = $state<FocalPoint[]>([]);
	imageName = $state('');
	imageWidth = $state(0);
	imageHeight = $state(0);
	selectedFrameId = $state<number | null>(null);
	selectedPointId = $state<number | null>(null);
	selection = $state<Set<number>>(new Set());
	pointSelection = $state<Set<number>>(new Set());
	compositionLabels = $state<Map<number, string>>(new Map());
	dirty = $state(false);
	historySeqs = $state<number[]>([]);
	historyPos = $state(0);

	// Nothing selected means every frame: the selection narrows the export, it does not gate it.
	framesToExport = $derived(
		this.selection.size > 0 ? this.frames.filter((f) => this.selection.has(f.id)) : this.frames
	);
	selectedFrames = $derived(this.frames.filter((f) => this.selection.has(f.id)));
	canUndo = $derived(undoSeq(this.historySeqs, this.historyPos) !== null);
	canRedo = $derived(redoSeq(this.historySeqs, this.historyPos) !== null);

	#pointAnchor: number | null = null;
	#store?: FramerStore;
	#onchange?: (doc: CropDoc) => void;
	#onerror?: (message: string) => void;
	#initialSeqs: number[];
	#initialPos: number;
	#restoring = false;
	#ready = false;
	#timer: ReturnType<typeof setTimeout> | null = null;

	constructor(options: SessionOptions) {
		this.frames = options.doc.frames;
		this.focalPoints = options.doc.focalPoints;
		this.imageName = options.doc.image.name;
		this.imageWidth = options.doc.image.width;
		this.imageHeight = options.doc.image.height;
		this.#store = options.store;
		this.#onchange = options.onchange;
		this.#onerror = options.onerror;
		this.#initialSeqs = options.historySeqs ?? [];
		this.#initialPos = options.historyPos ?? 0;
	}

	toDoc(): CropDoc {
		return {
			version: CROP_DOC_VERSION,
			image: { name: this.imageName, width: this.imageWidth, height: this.imageHeight },
			frames: this.frames,
			focalPoints: this.focalPoints
		};
	}

	async start() {
		this.historySeqs = this.#initialSeqs;
		this.historyPos = clampPos(this.#initialSeqs, this.#initialPos);
		if (this.historySeqs.length === 0) {
			await this.#pushHistory();
			this.dirty = false;
		}
		this.#ready = true;
	}

	setImageSize(width: number, height: number) {
		this.imageWidth = width;
		this.imageHeight = height;
	}

	markSaved() {
		this.dirty = false;
	}

	markDirty() {
		this.dirty = true;
	}

	// Selection

	selectFrame(frameId: number | null, mode: SelectMode = 'replace') {
		if (frameId === null) {
			this.deselect();
			return;
		}
		const next = nextSelection(this.selection, frameId, mode);
		this.selection = next.selection;
		this.selectedFrameId = next.anchor;
		this.#clearPoints();
	}

	// Picking a point drops the frame edit anchor but leaves the frame selection alone:
	// that selection is the export set, and clearing it would silently widen an export
	// to every frame.
	selectPoint(pointId: number | null, mode: PointSelectMode = 'replace') {
		if (pointId === null) {
			this.#clearPoints();
			return;
		}
		this.selectedFrameId = null;
		const next = nextPointSelection(
			{ selection: this.pointSelection, anchor: this.#pointAnchor, focus: this.selectedPointId },
			this.focalPoints,
			pointId,
			mode
		);
		this.pointSelection = next.selection;
		this.selectedPointId = next.focus;
		this.#pointAnchor = next.anchor;
	}

	deselect() {
		this.selectedFrameId = null;
		this.selection = new Set();
		this.#clearPoints();
	}

	#clearPoints() {
		this.selectedPointId = null;
		this.pointSelection = new Set();
		this.#pointAnchor = null;
	}

	selectNextFrame() {
		this.#cycleFrame(1);
	}

	selectPrevFrame() {
		this.#cycleFrame(-1);
	}

	#cycleFrame(delta: 1 | -1) {
		if (this.frames.length === 0) return;
		this.selectFrame(cycleFrameId(this.frames, this.selectedFrameId, delta));
	}

	selectNextPoint() {
		this.#cyclePoint(1);
	}

	selectPrevPoint() {
		this.#cyclePoint(-1);
	}

	#cyclePoint(delta: 1 | -1) {
		if (this.focalPoints.length === 0) return;
		this.selectPoint(cycleFrameId(this.focalPoints, this.selectedPointId, delta));
	}

	toggleFrameSelection(frameId: number) {
		this.selectFrame(frameId, 'toggle');
	}

	selectAllFrames() {
		this.selection = new Set(this.frames.map((f) => f.id));
		this.selectedFrameId = this.frames.at(-1)?.id ?? null;
		this.#clearPoints();
	}

	clearSelection() {
		this.deselect();
	}

	// Editing

	async addFrame(): Promise<number | null> {
		const frame: Omit<Frame, 'id'> = {
			label: undefined,
			destination: null,
			x: 0.2,
			y: 0.2,
			width: 0.6,
			height: 0.6,
			rotation: 0,
			is_default: false,
			is_auto: false
		};
		try {
			const id = await this.#insertFrame(frame);
			this.selectFrame(id);
			this.#emit();
			this.#capture();
			return id;
		} catch (error) {
			console.error('Failed to add frame:', error);
			this.#onerror?.('Failed to add frame');
			return null;
		}
	}

	async addPoint(x = 0.5, y = 0.5): Promise<number | null> {
		try {
			const id = await this.#insertPoint(x, y);
			this.#emit();
			this.#capture();
			return id;
		} catch (error) {
			console.error('Failed to add focal point:', error);
			this.#onerror?.('Failed to add focal point');
			return null;
		}
	}

	async deleteFrame(frameId: number) {
		return this.deleteFrames([frameId]);
	}

	// One journal entry for the whole batch, so one undo brings every frame back.
	async deleteFrames(frameIds: number[]): Promise<number> {
		const doomed = new Set(frameIds.filter((id) => this.frames.some((f) => f.id === id)));
		if (doomed.size === 0) return 0;
		for (const id of doomed) await this.#store?.deleteFrame(id);
		if (this.selectedFrameId !== null && doomed.has(this.selectedFrameId)) this.selectedFrameId = null;
		this.selection = new Set([...this.selection].filter((id) => !doomed.has(id)));
		this.frames = this.frames.filter((f) => !doomed.has(f.id));
		this.#emit();
		this.#capture();
		return doomed.size;
	}

	// The visible selection is what actions apply to; a lone anchor counts as a selection of one.
	async deleteSelectedFrames(): Promise<number> {
		const ids =
			this.selection.size > 0 ? [...this.selection] : this.selectedFrameId !== null ? [this.selectedFrameId] : [];
		return this.deleteFrames(ids);
	}

	async deletePoint(pointId: number) {
		return this.deletePoints([pointId]);
	}

	// One journal entry for the whole batch, so one undo brings every point back.
	async deletePoints(pointIds: number[]): Promise<number> {
		const doomed = new Set(pointIds.filter((id) => this.focalPoints.some((p) => p.id === id)));
		if (doomed.size === 0) return 0;
		for (const id of doomed) await this.#store?.deletePoint(id);
		if (this.selectedPointId !== null && doomed.has(this.selectedPointId)) this.selectedPointId = null;
		this.pointSelection = new Set([...this.pointSelection].filter((id) => !doomed.has(id)));
		this.focalPoints = this.focalPoints.filter((p) => !doomed.has(p.id));
		this.#emit();
		await this.recomputeAutoFrames();
		this.#capture();
		return doomed.size;
	}

	// Every path that focuses a point also puts it in the selection, so the selection alone
	// is the full answer to "what would delete remove".
	async deleteSelectedPoints(): Promise<number> {
		return this.deletePoints([...this.pointSelection]);
	}

	// A manual drag, resize or ratio change makes the frame the user's: it stops being recomputed.
	async claimFrame(frameId: number, geometry: Partial<Frame>) {
		const frame = this.frames.find((f) => f.id === frameId);
		if (!frame) return;
		const wasAuto = frame.is_auto;
		frame.is_auto = false;
		Object.assign(frame, geometry);
		await this.#store?.updateFrame(frameId, wasAuto ? { ...geometry, is_auto: false } : geometry);
		this.#emit();
	}

	async setFrameLabel(frameId: number, label: string) {
		const frame = this.frames.find((f) => f.id === frameId);
		if (!frame) return;
		frame.label = label;
		await this.#store?.updateFrame(frameId, { label });
		this.#emit();
	}

	async movePoint(pointId: number, position: { x: number; y: number }) {
		const point = this.focalPoints.find((p) => p.id === pointId);
		if (!point) return;
		point.x = position.x;
		point.y = position.y;
		await this.#store?.updatePoint(pointId, position);
		this.#emit();
	}

	// Records an edit the caller already applied: recompute the auto frames, then journal it.
	touch() {
		if (!this.#ready) return;
		void this.recomputeAutoFrames();
		this.#capture();
	}

	async generateVersions(slugs: string[]): Promise<GenerateResult | null> {
		if (slugs.length === 0) return { generated: 0, clipped: 0 };
		if (this.imageWidth === 0 || this.imageHeight === 0) return null;

		const imageAspect = this.imageWidth / this.imageHeight;
		const points = this.#globalFocalInputs();
		let clipped = 0;

		for (const slug of slugs) {
			const destination = findDestination(slug);
			if (!destination) continue;

			const aspect = destination.ratio.w / destination.ratio.h;
			const result = computeAutoFrame(points, aspect, imageAspect);
			if (!result.satisfiesRequired) clipped++;

			const existing = this.frames.find(
				(f) => f.is_auto && (f.destination === slug || (!f.destination && legacyAutoAspect(f.label) === aspect))
			);

			if (existing) {
				await this.#applyRect(existing, result.rect);
				existing.destination = slug;
				existing.label = destination.name;
				await this.#store?.updateFrame(existing.id, { destination: slug, label: destination.name });
			} else {
				await this.#insertFrame({
					label: destination.name,
					destination: slug,
					x: result.rect.x,
					y: result.rect.y,
					width: result.rect.width,
					height: result.rect.height,
					rotation: 0,
					is_default: false,
					is_auto: true
				});
			}
		}

		this.#emit();
		await this.recomputeAutoFrames();
		this.#capture();

		return { generated: slugs.length, clipped };
	}

	async recomputeAutoFrames() {
		if (this.imageWidth === 0 || this.imageHeight === 0) return;

		const points = this.#globalFocalInputs();
		const imageAspect = this.imageWidth / this.imageHeight;

		if (points.length > 0) {
			for (const frame of this.frames) {
				if (!frame.is_auto) continue;
				const result = computeAutoFrame(points, this.#autoTargetAspect(frame), imageAspect);
				await this.#applyRect(frame, result.rect);
			}
		}

		const labels = new Map<number, string>();
		for (const frame of this.frames) {
			const relevant = frame.is_auto
				? points
				: this.focalPoints.filter((p) => !p.frame_id || p.frame_id === frame.id).map((p) => ({ x: p.x, y: p.y }));
			if (relevant.length === 0) continue;
			const frameAspect = (frame.width * this.imageWidth) / (frame.height * this.imageHeight);
			const label = computeFrameLabel(
				{ x: frame.x, y: frame.y, width: frame.width, height: frame.height },
				relevant,
				frameAspect,
				imageAspect
			);
			if (label) labels.set(frame.id, label);
		}
		this.compositionLabels = labels;
	}

	// History

	async undo() {
		await this.#flush();
		await this.#restore(undoSeq(this.historySeqs, this.historyPos));
	}

	async redo() {
		await this.#flush();
		await this.#restore(redoSeq(this.historySeqs, this.historyPos));
	}

	async #restore(seq: number | null) {
		if (seq === null || this.#restoring) return;
		this.#restoring = true;
		try {
			const restored = await this.#store?.loadHistory(seq);
			if (!restored) return;
			this.historyPos = seq;
			this.selectedFrameId = remapId(this.selectedFrameId, restored.frameIds);
			this.#clearPoints();
			this.selection = remapSelection(this.selection, restored.frameIds);
			this.frames = restored.frames;
			this.focalPoints = restored.focalPoints;
			this.dirty = false;
			this.#emit();
		} catch (error) {
			console.error('Failed to restore history:', error);
			this.#onerror?.('Undo failed');
		} finally {
			this.#restoring = false;
		}
		await this.recomputeAutoFrames();
	}

	#capture() {
		this.dirty = true;
		if (this.#timer) clearTimeout(this.#timer);
		this.#timer = setTimeout(() => {
			this.#timer = null;
			void this.#pushHistory();
		}, HISTORY_DEBOUNCE_MS);
	}

	async #flush() {
		if (!this.#timer) return;
		clearTimeout(this.#timer);
		this.#timer = null;
		await this.#pushHistory();
	}

	async #pushHistory() {
		if (this.#restoring) return;
		const { drop, seq } = planAppend(this.historySeqs, this.historyPos);
		try {
			await this.#store?.saveHistory(seq, drop, serializeSnapshot(this.frames, this.focalPoints));
			this.historySeqs = this.historySeqs.filter((s) => !drop.includes(s)).concat(seq);
			this.historyPos = seq;
		} catch (error) {
			console.error('Failed to record history:', error);
		}
	}

	// Internals

	async #insertFrame(frame: Omit<Frame, 'id'>): Promise<number> {
		const id = (await this.#store?.addFrame(frame)) ?? nextLocalId(this.toDoc());
		this.frames.push({ id, ...frame });
		return id;
	}

	async #insertPoint(x: number, y: number): Promise<number> {
		const point: Omit<FocalPoint, 'id'> = {
			frame_id: undefined,
			x,
			y,
			label: `Point ${this.focalPoints.length + 1}`,
			required: true
		};
		const id = (await this.#store?.addPoint(point)) ?? nextLocalId(this.toDoc());
		this.focalPoints.push({ id, ...point });
		return id;
	}

	async #applyRect(frame: Frame, rect: { x: number; y: number; width: number; height: number }) {
		frame.x = rect.x;
		frame.y = rect.y;
		frame.width = rect.width;
		frame.height = rect.height;
		await this.#store?.updateFrame(frame.id, rect);
	}

	#globalFocalInputs() {
		return this.focalPoints.filter((p) => !p.frame_id).map((p) => ({ x: p.x, y: p.y, required: p.required }));
	}

	#autoTargetAspect(frame: Frame): number {
		const destination = findDestination(frame.destination);
		if (destination) return destination.ratio.w / destination.ratio.h;
		const legacy = legacyAutoAspect(frame.label);
		if (legacy !== null) return legacy;
		return (frame.width * this.imageWidth) / (frame.height * this.imageHeight);
	}

	#emit() {
		this.#onchange?.(this.toDoc());
	}
}
