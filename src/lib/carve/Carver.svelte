<script lang="ts">
// The carve screen: a photo, a working-size carve in a worker drawn seam by seam, and knobs. The
// size slider is instant once the carve has run to 60% smaller. Shorter carves the transposed
// photo by columns; only drawing and pointer input swap the axes back.
import { onMount } from 'svelte';
import { useDock } from '$lib/components/dock.svelte';
import { loadImage } from '$lib/framer/canvas-export';
import {
	type Band,
	type ColumnMap,
	clamp,
	columnMap,
	costMap,
	imageDataOf,
	lumaOf,
	mapPoint,
	orient,
	orientPoint,
	outputWidth,
	type Plane,
	type Rgb,
	renderResult,
	resultFraction,
	seamsFor,
	shade,
	startCarve,
	target,
	tint,
	transpose,
	type Work,
	workSize
} from './carve';
import {
	type CarvePoint,
	type CarveSettings,
	DEFAULT_SETTINGS,
	type Energy,
	MAX_POINT_SIZE,
	MIN_POINT_SIZE
} from './types';

export interface CarveResult {
	pixels: ImageData;
	width: number;
	height: number;
	settings: CarveSettings;
	points: CarvePoint[];
	ratio: string;
	/** Where an original position lands in the version, or null when the carve removed it. */
	map: (p: { x: number; y: number }) => { x: number; y: number } | null;
}

interface Props {
	imageSrc: string;
	title: string;
	backHref?: string;
	initialPoints?: CarvePoint[];
	onsave?: (result: CarveResult) => Promise<void>;
	onexport?: (result: CarveResult) => Promise<void>;
}

let { imageSrc, title, backHref, initialPoints = [], onsave, onexport }: Props = $props();

type View = 'original' | 'crop' | 'result' | 'seams';
const VIEWS: View[] = ['original', 'crop', 'result', 'seams'];
const PREVIEW = 1600;
const BACKGROUND = 0xff1a1a1a;
const GREEN: Rgb = [74, 222, 128];
const SIZE_STEP = 1.15;
const TRANSITION_MS = 450;
const TRAIL_MS = 220;
const MAX_TRAIL = 12;
const CACHE_LIMIT = 12;

const rgb = (hex: string): Rgb => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as Rgb;
const strengthText = (s: number) => (s === 0 ? 'off' : s === 10 ? 'never cross' : `soft ${s}`);
const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
const ease = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);

let view = $state<View>('result');
let lastView: View = 'result';
let settings = $state<CarveSettings>({ ...DEFAULT_SETTINGS });
// svelte-ignore state_referenced_locally
let points = $state<CarvePoint[]>(initialPoints.map((p) => ({ ...p })));
let selected = $state<CarvePoint | null>(null);
let brush = $state<CarvePoint>({ kind: 'protect', x: 0, y: 0, size: 0.12, strength: 10 });
let showPoints = $state(true);
let seamColour = $state('#22d3ee');
let glow = $state(true);
let fxSize = $state(100);
let loaded = $state(false);
let failed = $state(false);
let carving = $state(false);
let steps = $state(0);
let maxSeams = $state(0);
let elapsed = $state(0);
let fromCache = $state(false);
let workLabel = $state('');
let busy = $state<'save' | 'export' | null>(null);
let phone = $state(false);

let canvas: HTMLCanvasElement;
let marks: HTMLCanvasElement;
let fx: HTMLCanvasElement;
let context: CanvasRenderingContext2D;
let marksContext: CanvasRenderingContext2D;
let fxContext: CanvasRenderingContext2D;

let image: HTMLImageElement | null = null;
let preview: Plane | null = null;
let oriented: Plane | null = null;
let work: Work | null = null;
let rank: Uint16Array | null = null;
let cancelCarve: (() => void) | null = null;
let replayStep: number | null = null;
let drawQueued = false;
let carveTimer: ReturnType<typeof setTimeout> | undefined;
let hover: [number, number] | null = null;
let alt = false;
let drag: { point: CarvePoint; moved: boolean } | null = null;

const rows = () => settings.direction === 'rows';
const removed = $derived(Math.round((1 - settings.size) * 100));
const sizeText = $derived(
	settings.size < 1
		? `${Math.round(settings.size * 100)}% · ${removed}% removed`
		: settings.size > 1
			? `${Math.round(settings.size * 100)}% · widened`
			: '100%'
);
const status = $derived(
	!loaded
		? 'Loading…'
		: carving
			? `Carving ${steps}/${maxSeams}`
			: `${(elapsed / 1000).toFixed(2)} s${fromCache ? ', kept' : ''} · ${workLabel}`
);
const edgeWords = $derived(rows() ? ['Top edge', 'Bottom edge', 'height'] : ['Left edge', 'Right edge', 'width']);
const leanText = $derived.by(() => {
	const [before, after] = rows() ? ['top', 'bottom'] : ['left', 'right'];
	const lean = settings.lean;
	return lean === 0 ? 'none' : lean < 0 ? `cut ${before} ${-lean}` : `cut ${after} ${lean}`;
});
const editing = $derived(selected ?? brush);

useDock({ title: dockTitle, islands: dockIslands });

// ---- points

function changePoint(change: (point: CarvePoint) => void) {
	const point = selected ?? brush;
	change(point);
	point.size = clamp(point.size, MIN_POINT_SIZE, MAX_POINT_SIZE);
	point.strength = clamp(Math.round(point.strength), 0, 10);
	drawMarks();
	if (point !== brush) scheduleCarve();
}

function select(point: CarvePoint | null) {
	selected = point;
	drawMarks();
}

function deleteSelected() {
	if (!selected) return;
	points.splice(points.indexOf(selected), 1);
	select(null);
	scheduleCarve();
}

function clearPoints() {
	points = [];
	select(null);
	scheduleCarve();
}

function setShowPoints(show: boolean) {
	showPoints = show;
	if (!show) selected = null;
	drawMarks();
}

// ---- pixels

function pixelsAt(source: HTMLImageElement, w: number, h: number): Plane {
	const scratch = new OffscreenCanvas(w, h).getContext('2d');
	if (!scratch) throw new Error('Canvas 2D is unavailable');
	scratch.drawImage(source, 0, 0, w, h);
	return { w, h, pixels: new Uint32Array(scratch.getImageData(0, 0, w, h).data.buffer) };
}

function prepareWork(): { work: Work; luma: Float32Array } {
	if (!image) throw new Error('no image');
	const size = workSize(image.naturalWidth, image.naturalHeight, settings.working);
	const plane = orient(pixelsAt(image, size.w, size.h), rows());
	return { work: { w: plane.w, h: plane.h }, luma: lumaOf(plane.pixels) };
}

// ---- carving. Finished carves are kept by everything that shapes them, so flipping the energy
// or going back to a setting already tried shows the result at once.
// ponytail: a dozen entries of about 1 MB each; evict oldest first.
const carveCache = new Map<string, { work: Work; rank: Uint16Array; maxSeams: number; elapsed: number }>();

function carveKey() {
	const { size: _size, ...shape } = settings;
	return JSON.stringify([shape, $state.snapshot(points)]);
}

function carve() {
	if (!image) return;
	cancelCarve?.();
	cancelCarve = null;
	replayStep = null;
	const key = carveKey();
	const cached = carveCache.get(key);
	if (cached) {
		({ work, rank, maxSeams, elapsed } = cached);
		steps = maxSeams;
		carving = false;
		fromCache = true;
		workLabel = `${work.w}×${work.h}`;
		requestDraw();
		return;
	}
	fromCache = false;
	const prepared = prepareWork();
	work = prepared.work;
	const { w, h } = work;
	const carveWork = work;
	const carveRank = new Uint16Array(w * h).fill(0xffff);
	rank = carveRank;
	steps = 0;
	maxSeams = seamsFor(work);
	workLabel = `${w}×${h}`;
	carving = true;
	const { energy, straight, blur } = settings;
	cancelCarve = startCarve(
		{
			w,
			h,
			luma: prepared.luma,
			cost: costMap(work, settings, points, rows()),
			energy,
			straight,
			blur,
			seams: maxSeams
		},
		(step, seam, took) => {
			for (let y = 0; y < h; y++) carveRank[y * w + seam[y]] = step;
			steps = step + 1;
			elapsed = took;
			requestDraw();
		},
		(took) => {
			elapsed = took;
			carving = false;
			cancelCarve = null;
			carveCache.set(key, { work: carveWork, rank: carveRank, maxSeams, elapsed });
			if (carveCache.size > CACHE_LIMIT) carveCache.delete(carveCache.keys().next().value as string);
			requestDraw();
		}
	);
	requestDraw();
}

function scheduleCarve() {
	clearTimeout(carveTimer);
	carveTimer = setTimeout(carve, 180);
}

const wanted = () => target(settings.size, work as Work, maxSeams);

// ---- painting the photo

interface Geometry {
	kind: 'original' | 'result';
	offset: number;
	left: number;
	right: number;
	outW?: number;
	length?: number;
	map?: Float32Array;
	n: number;
	e: number;
}

let geometry: Geometry | null = null;
let frameCache: { width: number; height: number; buffer: Uint32Array } | null = null;

function band(): Band | null {
	const { from, to } = settings;
	return from > 0 || to < 1 ? { from, to } : null;
}

// The canvas stays the original's size while shrinking, so the photo keeps its scale, and grows
// only when the photo is widened.
function frame() {
	const plane = oriented as Plane;
	const width = Math.ceil(plane.w * Math.max(1, settings.size)) + 2;
	const height = plane.h;
	if (frameCache?.width !== width || frameCache.height !== height) {
		frameCache = { width, height, buffer: new Uint32Array(width * height) };
	}
	frameCache.buffer.fill(BACKGROUND);
	return frameCache;
}

function paintResult(out: ReturnType<typeof frame>, layout: ColumnMap): Geometry {
	const plane = oriented as Plane;
	const result = renderResult(plane, work as Work, layout, band());
	const offset = Math.round((out.width - result.w) / 2);
	for (let Y = 0; Y < result.h; Y++) {
		out.buffer.set(result.pixels.subarray(Y * result.w, (Y + 1) * result.w), Y * out.width + offset);
	}
	return {
		kind: 'result',
		offset,
		left: offset,
		right: offset + result.w,
		outW: result.w,
		length: layout.length,
		map: layout.map,
		n: 0,
		e: 0
	};
}

// The original, optionally tinted where the current size removes (or doubles) columns.
function paintOriginal(out: ReturnType<typeof frame>, n: number, e: number, tinted: boolean): Geometry {
	const { w, h } = work as Work;
	const { w: dw, h: dh, pixels } = oriented as Plane;
	const ranks = rank as Uint16Array;
	const offset = Math.round((out.width - dw) / 2);
	const removedColour = rgb(seamColour);
	const region = band();
	for (let Y = 0; Y < dh; Y++) {
		const wy = Math.min(((Y * h) / dh) | 0, h - 1) * w;
		const row = Y * out.width + offset;
		for (let X = 0; X < dw; X++) {
			let p = pixels[Y * dw + X];
			if (tinted) {
				const r = ranks[wy + Math.min(((X * w) / dw) | 0, w - 1)];
				if (r < n) p = tint(p, removedColour);
				else if (r < e) p = tint(p, GREEN);
			}
			if (region && (X < region.from * dw || X > region.to * dw)) p = shade(p);
			out.buffer[row + X] = p;
		}
	}
	return { kind: 'original', offset, left: offset, right: offset + dw, n: 0, e: 0 };
}

function cropStart(outW: number) {
	const plane = oriented as Plane;
	const protect = points.filter((p) => p.kind === 'protect').map((p) => orientPoint(p, rows()));
	const centre = protect.length ? protect.reduce((s, p) => s + p.x, 0) / protect.length : 0.5;
	return clamp(Math.round(centre * plane.w - outW / 2), 0, plane.w - outW);
}

function paintCrop(out: ReturnType<typeof frame>, n: number): Geometry {
	const { w: dw, h: dh, pixels } = oriented as Plane;
	const { w } = work as Work;
	const outW = Math.round((dw * (w - n)) / w);
	const start = cropStart(outW);
	const offset = Math.round((out.width - outW) / 2);
	const region = band();
	for (let Y = 0; Y < dh; Y++) {
		for (let X = 0; X < outW; X++) {
			let p = pixels[Y * dw + start + X];
			if (region && (start + X < region.from * dw || start + X > region.to * dw)) p = shade(p);
			out.buffer[Y * out.width + offset + X] = p;
		}
	}
	return { kind: 'original', offset: offset - start, left: offset, right: offset + outW, n: 0, e: 0 };
}

// ---- mapping between the original photo and canvas pixels in the current view

function toCanvas(point: { x: number; y: number }): [number, number] {
	const g = geometry as Geometry;
	const plane = oriented as Plane;
	const o = orientPoint(point, rows());
	let x: number;
	if (g.kind === 'result') {
		const f = resultFraction(o, rank as Uint16Array, work as Work, g.n, g.e);
		x = g.offset + (f ?? o.x) * (g.outW as number);
	} else x = g.offset + o.x * plane.w;
	const y = o.y * plane.h;
	return rows() ? [y, x] : [x, y];
}

function fromCanvas([cx, cy]: [number, number]) {
	const g = geometry as Geometry;
	const [X, Y] = rows() ? [cy, cx] : [cx, cy];
	const { w: dw, h: dh } = oriented as Plane;
	if (X < g.left || X > g.right || Y < 0 || Y > dh) return null;
	let ox: number;
	if (g.kind === 'result') {
		const { w, h } = work as Work;
		const length = g.length as number;
		const i = clamp(Math.round(((X - g.offset + 0.5) * length) / (g.outW as number) - 0.5), 0, length - 1);
		const row = clamp(Math.round(((Y + 0.5) * h) / dh - 0.5), 0, h - 1);
		ox = ((g.map as Float32Array)[row * length + i] + 0.5) / w;
	} else {
		ox = (X - g.offset) / dw;
	}
	const oy = Y / dh;
	return rows() ? { x: oy, y: ox } : { x: ox, y: oy };
}

function eventToCanvas(event: PointerEvent | WheelEvent): [number, number] {
	const box = canvas.getBoundingClientRect();
	return [
		((event.clientX - box.left) * canvas.width) / box.width,
		((event.clientY - box.top) * canvas.height) / box.height
	];
}

const shortSide = () => Math.min((oriented as Plane).w, (oriented as Plane).h);

function pointAt(position: [number, number] | null): CarvePoint | null {
	if (!position || !showPoints || !geometry) return null;
	for (let i = points.length - 1; i >= 0; i--) {
		const point = points[i];
		const [x, y] = toCanvas(point);
		if (Math.hypot(x - position[0], y - position[1]) <= point.size * shortSide()) return point;
	}
	return null;
}

// ---- marks: points and the brush outline, on their own canvas so the cursor moves without
// repainting the photo

function circle(g: CanvasRenderingContext2D, x: number, y: number, radius: number) {
	g.beginPath();
	g.arc(x, y, radius, 0, Math.PI * 2);
	g.stroke();
}

function drawPoint(g: CanvasRenderingContext2D, point: CarvePoint, scale: number) {
	const [x, y] = toCanvas(point);
	const radius = point.size * shortSide();
	const remove = point.kind === 'remove';
	const isSelected = point === selected;
	g.setLineDash(remove ? [6 * scale, 5 * scale] : point.strength < 10 ? [2 * scale, 4 * scale] : []);
	g.globalAlpha = remove ? 1 : 0.45 + 0.055 * point.strength;
	g.lineWidth = (isSelected ? 3 : 2) * scale;
	g.strokeStyle = 'rgba(0, 0, 0, .5)';
	circle(g, x, y, radius + g.lineWidth);
	g.strokeStyle = isSelected ? '#60a5fa' : remove ? '#f87171' : '#fff';
	circle(g, x, y, radius);
	g.globalAlpha = 1;
	g.setLineDash([]);
	g.fillStyle = g.strokeStyle;
	g.beginPath();
	g.arc(x, y, 3.5 * scale, 0, Math.PI * 2);
	g.fill();
	if (!isSelected) return;
	const label = `${Math.round(point.size * 100)}% · ${remove ? 'remove' : strengthText(point.strength)}`;
	g.font = `${12 * scale}px ui-sans-serif, system-ui, sans-serif`;
	const width = g.measureText(label).width + 12 * scale;
	g.fillStyle = 'rgba(0, 0, 0, .75)';
	g.fillRect(x - width / 2, y - radius - 26 * scale, width, 20 * scale);
	g.fillStyle = '#fff';
	g.textAlign = 'center';
	g.textBaseline = 'middle';
	g.fillText(label, x, y - radius - 16 * scale);
}

function drawBrush(g: CanvasRenderingContext2D, scale: number) {
	const [x, y] = hover as [number, number];
	g.setLineDash([4 * scale, 4 * scale]);
	g.lineWidth = 1.5 * scale;
	g.strokeStyle = brush.kind === 'remove' || alt ? '#f87171' : 'rgba(255, 255, 255, .8)';
	circle(g, x, y, brush.size * shortSide());
	g.setLineDash([]);
}

function drawMarks() {
	if (!marksContext) return;
	syncLayers();
	const g = marksContext;
	g.clearRect(0, 0, marks.width, marks.height);
	if (!geometry || transition) return;
	const scale = marks.width / Math.max(marks.clientWidth, 1);
	if (showPoints) for (const point of points) drawPoint(g, point, scale);
	const overPoint = pointAt(hover);
	canvas.style.cursor = overPoint || drag ? 'move' : 'crosshair';
	if (hover && !overPoint && !drag) drawBrush(g, scale);
}

function syncLayers() {
	for (const layer of [marks, fx]) {
		if (layer.width !== canvas.width || layer.height !== canvas.height)
			[layer.width, layer.height] = [canvas.width, canvas.height];
		layer.style.left = `${canvas.offsetLeft}px`;
		layer.style.top = `${canvas.offsetTop}px`;
		layer.style.width = `${canvas.offsetWidth}px`;
		layer.style.height = `${canvas.offsetHeight}px`;
	}
}

// ---- the seam lives on its own canvas over the photo, so the glow keeps moving between photo
// redraws. Paths are flat [x0, y0, x1, y1, …] in canvas pixels.

const effects = {
	seam: null as number[] | null,
	key: null as number | null,
	photo: null as [number, number, number, number] | null,
	trail: [] as { path: number[]; age: number }[],
	running: false,
	last: 0
};

function seamPath(marked: Int32Array): number[] {
	const g = geometry as Geometry;
	const dh = (oriented as Plane).h;
	const h = (work as Work).h;
	const path: number[] = [];
	for (let y = 0; y < h; y++) {
		if (marked[y] < 0) continue;
		const x = g.offset + ((marked[y] + 0.5) * (g.outW as number)) / (g.length as number);
		const Y = y === 0 ? 0 : y === h - 1 ? dh : ((y + 0.5) * dh) / h;
		if (rows()) path.push(Y, x);
		else path.push(x, Y);
	}
	return path;
}

// A seam that gives way to the next one fades out as part of the trail. `photo` is the result's
// rectangle; the trail stays inside it while the photo shrinks.
function setSeam(path: number[] | null, key: number | null, photo: [number, number, number, number] | null = null) {
	if (photo) effects.photo = photo;
	if (effects.seam && key !== effects.key) {
		effects.trail.push({ path: effects.seam, age: 0 });
		if (effects.trail.length > MAX_TRAIL) effects.trail.shift();
	}
	effects.seam = path;
	effects.key = path ? key : null;
	if (effects.running) return;
	effects.running = true;
	effects.last = performance.now();
	requestAnimationFrame(animateFx);
}

// Jitter is a damped random walk across the seam, which reads as lightning rather than fuzz.
function trace(g: CanvasRenderingContext2D, path: number[], jitter: number) {
	const across = rows() ? 1 : 0;
	let shake = 0;
	g.beginPath();
	for (let k = 0; k < path.length; k += 2) {
		if (jitter) shake = shake * 0.8 + (Math.random() - 0.5) * jitter;
		const x = path[k] + (across === 0 ? shake : 0);
		const y = path[k + 1] + (across === 1 ? shake : 0);
		if (k === 0) g.moveTo(x, y);
		else g.lineTo(x, y);
	}
	g.stroke();
}

function drawZap(g: CanvasRenderingContext2D, path: number[], colour: string, scale: number) {
	g.save();
	g.globalCompositeOperation = 'lighter';
	g.lineJoin = 'round';
	g.globalAlpha = 0.75 + Math.random() * 0.25;
	g.strokeStyle = colour;
	g.shadowColor = colour;
	g.shadowBlur = 16 * scale;
	g.lineWidth = 4 * scale;
	trace(g, path, 5 * scale);
	g.shadowBlur = 6 * scale;
	g.lineWidth = 2 * scale;
	trace(g, path, 3 * scale);
	g.shadowBlur = 0;
	g.strokeStyle = '#fff';
	g.lineWidth = scale;
	trace(g, path, 2 * scale);
	g.restore();
}

function drawTrail(g: CanvasRenderingContext2D, dt: number, colour: string, scale: number) {
	for (const item of effects.trail) item.age += dt;
	effects.trail = effects.trail.filter((item) => item.age < TRAIL_MS);
	g.save();
	if (effects.photo) {
		g.beginPath();
		g.rect(...effects.photo);
		g.clip();
	}
	g.globalCompositeOperation = 'lighter';
	g.strokeStyle = colour;
	g.lineWidth = 1.5 * scale;
	for (const item of effects.trail) {
		g.globalAlpha = 0.45 * (1 - item.age / TRAIL_MS);
		trace(g, item.path, 0);
	}
	g.restore();
}

function animateFx(now: number) {
	const dt = Math.min(now - effects.last, 50);
	effects.last = now;
	const g = fxContext;
	g.clearRect(0, 0, fx.width, fx.height);
	const scale = (fx.width / Math.max(fx.clientWidth, 1)) * (fxSize / 100);
	if (glow) {
		drawTrail(g, dt, seamColour, scale);
		if (effects.seam) drawZap(g, effects.seam, seamColour, scale);
	} else {
		effects.trail = [];
		if (effects.seam) {
			g.strokeStyle = seamColour;
			g.lineWidth = 1.5 * scale;
			trace(g, effects.seam, 0);
		}
	}
	if (effects.seam || effects.trail.length) requestAnimationFrame(animateFx);
	else effects.running = false;
}

// ---- one frame of the photo

function blit(out: ReturnType<typeof frame>) {
	const size = rows() ? [out.height, out.width] : [out.width, out.height];
	if (canvas.width !== size[0] || canvas.height !== size[1]) [canvas.width, canvas.height] = size;
	const data = imageDataOf({ w: out.width, h: out.height, pixels: out.buffer });
	if (rows()) {
		const turned = new OffscreenCanvas(out.width, out.height);
		turned.getContext('2d')?.putImageData(data, 0, 0);
		context.setTransform(0, 1, 1, 0, 0, 0);
		context.drawImage(turned, 0, 0);
		context.setTransform(1, 0, 0, 1, 0, 0);
	} else {
		context.putImageData(data, 0, 0);
	}
}

// A transition paints its own frames; the exact view is drawn once it ends.
function draw() {
	drawQueued = false;
	if (!work || !rank || transition) return;
	const asked = wanted();
	let n = asked.n;
	let e = asked.e;
	let showSeam = false;
	if (replayStep !== null) {
		n = Math.min(replayStep, asked.n);
		showSeam = replayStep < asked.n;
	} else if (carving) {
		if (n > 0 && steps <= n) {
			n = Math.max(steps - 1, 0);
			showSeam = steps > 0;
		}
		e = Math.min(e, steps);
	}
	const out = frame();
	let layout: ColumnMap | null = null;
	if (view === 'original') geometry = paintOriginal(out, 0, 0, false);
	else if (view === 'seams') geometry = paintOriginal(out, n, e, true);
	else if (view === 'crop' && e === 0) geometry = paintCrop(out, n);
	else {
		layout = columnMap(rank, work, n, e);
		geometry = paintResult(out, layout);
	}
	geometry.n = n;
	geometry.e = e;
	blit(out);
	drawMarks();
	const seam = layout && showSeam ? seamPath(layout.marked) : null;
	const rect: [number, number, number, number] | null =
		geometry.kind === 'result' ? [geometry.offset, 0, geometry.outW as number, (oriented as Plane).h] : null;
	setSeam(seam, n, rect && rows() ? [rect[1], rect[0], rect[3], rect[2]] : rect);
}

function requestDraw() {
	if (drawQueued) return;
	drawQueued = true;
	requestAnimationFrame(draw);
}

// ---- views, with a transition: every view places each working column of the original somewhere
// in each working row; a transition moves every column from one place to the other.

let transition: object | null = null;

function setView(next: View) {
	const previous = view;
	if (next !== 'original') lastView = next;
	view = next;
	if (previous === next || !geometry || carving || replayStep !== null) return requestDraw();
	morphViews(previous, next);
}

interface Placement {
	prefix: Float32Array;
	left: number;
	right: number;
	tint: number;
}

// prefix[y * (w + 1) + x] is the left edge of column x, in oriented canvas pixels.
function placement(of: View, n: number, e: number, canvasWidth: number): Placement {
	const { w, h } = work as Work;
	const ranks = rank as Uint16Array;
	const dw = (oriented as Plane).w;
	const prefix = new Float32Array(h * (w + 1));
	if (of === 'result' || (of === 'crop' && e > 0)) {
		const length = w - n + e;
		const outW = outputWidth(oriented as Plane, work as Work, length);
		const offset = Math.round((canvasWidth - outW) / 2);
		const unit = outW / length;
		for (let y = 0; y < h; y++) {
			const row = y * (w + 1);
			let at = offset;
			for (let x = 0; x < w; x++) {
				prefix[row + x] = at;
				const r = ranks[y * w + x];
				if (r >= n) at += (r < e ? 2 : 1) * unit;
			}
			prefix[row + w] = at;
		}
		return { prefix, left: offset, right: offset + outW, tint: 0 };
	}
	let offset = Math.round((canvasWidth - dw) / 2);
	let left = offset;
	let right = offset + dw;
	if (of === 'crop') {
		const outW = Math.round((dw * (w - n)) / w);
		left = Math.round((canvasWidth - outW) / 2);
		right = left + outW;
		offset = left - cropStart(outW);
	}
	const unit = dw / w;
	for (let y = 0; y < h; y++) for (let x = 0; x <= w; x++) prefix[y * (w + 1) + x] = offset + x * unit;
	return { prefix, left, right, tint: of === 'seams' ? 1 : 0 };
}

function paintMorph(out: ReturnType<typeof frame>, from: Placement, to: Placement, t: number, n: number, e: number) {
	const { w, h } = work as Work;
	const ranks = rank as Uint16Array;
	const { w: dw, h: dh, pixels } = oriented as Plane;
	const stride = w + 1;
	const start = Math.max(0, Math.ceil(lerp(from.left, to.left, t)));
	const end = Math.min(out.width, Math.floor(lerp(from.right, to.right, t)));
	const tinted = lerp(from.tint, to.tint, t);
	const removedColour = rgb(seamColour);
	const region = band();
	const edge = (row: number, x: number) => lerp(from.prefix[row + x], to.prefix[row + x], t);
	for (let Y = 0; Y < dh; Y++) {
		const y = Math.min((((Y + 0.5) * h) / dh) | 0, h - 1);
		const row = y * stride;
		const source = Y * dw;
		const target = Y * out.width;
		let x = 0;
		let left = edge(row, 0);
		let right = edge(row, 1);
		for (let X = start; X < end; X++) {
			const centre = X + 0.5;
			while (x < w - 1 && right <= centre) {
				x++;
				left = right;
				right = edge(row, x + 1);
			}
			if (centre < left || centre >= right) continue;
			const u = (centre - left) / (right - left);
			let p = pixels[source + (Math.min(((x + u) * dw) / w, dw - 1) | 0)];
			if (tinted > 0) {
				const r = ranks[y * w + x];
				if (r < n) p = tint(p, removedColour, tinted);
				else if (r < e) p = tint(p, GREEN, tinted);
			}
			if (region) {
				const across = (x + 0.5) / w;
				if (across < region.from || across > region.to) p = shade(p);
			}
			out.buffer[target + X] = p;
		}
	}
}

function morphViews(fromView: View, toView: View) {
	const { n, e } = wanted();
	const width = frame().width;
	const from = placement(fromView, n, e, width);
	const to = placement(toView, n, e, width);
	const started = performance.now();
	const token = {};
	transition = token;
	setSeam(null, null);
	drawMarks();
	const step = (now: number) => {
		if (transition !== token) return;
		const t = Math.min((now - started) / TRANSITION_MS, 1);
		const out = frame();
		paintMorph(out, from, to, ease(t), n, e);
		blit(out);
		if (t < 1) return requestAnimationFrame(step);
		transition = null;
		draw();
	};
	requestAnimationFrame(step);
}

// ---- pointer: click adds, drag moves, the wheel sizes the point under it or the brush

function pointerdown(event: PointerEvent) {
	if (!work || !geometry || event.button !== 0) return;
	const position = eventToCanvas(event);
	const hit = pointAt(position);
	if (hit) {
		select(hit);
		drag = { point: hit, moved: false };
		canvas.setPointerCapture(event.pointerId);
		return;
	}
	const at = fromCanvas(position);
	if (!at) return select(null);
	points.push({ ...at, kind: event.altKey ? 'remove' : brush.kind, size: brush.size, strength: brush.strength });
	showPoints = true;
	select(points[points.length - 1]);
	scheduleCarve();
}

function pointermove(event: PointerEvent) {
	hover = eventToCanvas(event);
	alt = event.altKey;
	if (drag) {
		const at = fromCanvas(hover);
		if (at) {
			Object.assign(drag.point, at);
			drag.moved = true;
		}
	}
	drawMarks();
}

function pointerup() {
	if (drag?.moved) scheduleCarve();
	drag = null;
	drawMarks();
}

function pointerleave() {
	hover = null;
	drawMarks();
}

function wheel(event: WheelEvent) {
	if (!geometry) return;
	event.preventDefault();
	const factor = event.deltaY < 0 ? SIZE_STEP : 1 / SIZE_STEP;
	const hit = pointAt(eventToCanvas(event));
	if (hit && hit !== selected) select(hit);
	if (!hit && selected) select(null);
	changePoint((p) => (p.size *= factor));
}

// Svelte makes wheel handlers passive, and this one has to stop the page scrolling.
function wheelListener(node: HTMLElement) {
	node.addEventListener('wheel', wheel, { passive: false });
	return () => node.removeEventListener('wheel', wheel);
}

// ---- keyboard, after Photoshop and Lightroom: [ ] size, shift [ ] hardness, H pins, \ before

function keydown(event: KeyboardEvent) {
	if (event.metaKey || event.ctrlKey || (event.target as HTMLElement).closest('select, input[type=color], textarea'))
		return;
	switch (event.code) {
		case 'BracketLeft':
			changePoint((p) => {
				if (event.shiftKey) p.strength -= 1;
				else p.size /= SIZE_STEP;
			});
			break;
		case 'BracketRight':
			changePoint((p) => {
				if (event.shiftKey) p.strength += 1;
				else p.size *= SIZE_STEP;
			});
			break;
		case 'Backspace':
		case 'Delete':
			if (!selected) return;
			deleteSelected();
			break;
		case 'Escape':
			select(null);
			break;
		case 'KeyH':
			setShowPoints(!showPoints);
			break;
		case 'Backslash':
			setView(view === 'original' ? lastView : 'original');
			break;
		case 'KeyE':
			setEnergy(settings.energy === 'forward' ? 'gradient' : 'forward');
			break;
		case 'Digit1':
		case 'Digit2':
		case 'Digit3':
		case 'Digit4':
			setView(VIEWS[Number(event.code.slice(-1)) - 1]);
			break;
		// A focused slider or radio uses the arrows itself.
		case 'ArrowLeft':
		case 'ArrowRight': {
			if ((event.target as HTMLElement).closest('input')) return;
			const step = event.code === 'ArrowRight' ? 1 : -1;
			setView(VIEWS[(VIEWS.indexOf(view) + step + VIEWS.length) % VIEWS.length]);
			break;
		}
		default:
			return;
	}
	event.preventDefault();
}

// ---- replay: the same seams again, one per frame, without carving

function replay() {
	if (carving || !work) return;
	const { n } = wanted();
	if (n === 0) return;
	setView(view === 'original' ? 'result' : view);
	const perFrame = Math.max(1, Math.round(n / 120));
	replayStep = 0;
	const tick = () => {
		replayStep = (replayStep as number) + perFrame;
		if (replayStep >= n) replayStep = null;
		draw();
		if (replayStep !== null) requestAnimationFrame(tick);
	};
	requestAnimationFrame(tick);
}

// ---- settings

function setEnergy(energy: Energy) {
	settings.energy = energy;
	carve();
}

function setDirection(direction: CarveSettings['direction']) {
	if (settings.direction === direction) return;
	settings.direction = direction;
	reorient();
}

function reorient() {
	if (!preview) return;
	oriented = orient(preview, rows());
	geometry = null;
	carve();
}

// The two region edges never cross: moving one past the other pushes it along.
function regionInput() {
	if (settings.from > settings.to) settings.to = settings.from;
	shapeInput();
}

function shapeInput() {
	requestDraw();
	scheduleCarve();
}

// ---- the version at the source's resolution
// ponytail: the full-size source and output sit in the webview as RGBA, about 8 bytes a pixel,
// and the canvas drops the colour profile to sRGB. A Rust render_carve taking the rank map is the
// upgrade when photos over 50 MP or Display P3 matter.
function renderFull(): CarveResult {
	if (!image || !work || !rank) throw new Error('nothing carved yet');
	const { n, e } = wanted();
	const ranks = rank;
	const size = work;
	const turned = rows();
	const full = orient(pixelsAt(image, image.naturalWidth, image.naturalHeight), turned);
	const result = renderResult(full, size, columnMap(ranks, size, n, e));
	const plane = turned ? { w: result.h, h: result.w, pixels: transpose(result.pixels, result.w, result.h) } : result;
	return {
		pixels: imageDataOf(plane),
		width: plane.w,
		height: plane.h,
		settings: $state.snapshot(settings),
		points: $state.snapshot(points),
		ratio: (plane.w / plane.h).toFixed(2),
		map: (p) => mapPoint(p, ranks, size, n, e, turned)
	};
}

async function run(kind: 'save' | 'export') {
	const handler = kind === 'save' ? onsave : onexport;
	if (!handler || busy || carving || !work) return;
	busy = kind;
	await new Promise((resolve) => requestAnimationFrame(() => setTimeout(resolve)));
	try {
		await handler(renderFull());
	} finally {
		busy = null;
	}
}

onMount(() => {
	context = canvas.getContext('2d') as CanvasRenderingContext2D;
	marksContext = marks.getContext('2d') as CanvasRenderingContext2D;
	fxContext = fx.getContext('2d') as CanvasRenderingContext2D;
	const observer = new ResizeObserver(() => drawMarks());
	observer.observe(canvas);
	const mq = matchMedia('(max-width: 700px)');
	phone = mq.matches;
	const onChange = () => (phone = mq.matches);
	mq.addEventListener('change', onChange);
	let alive = true;
	loadImage(imageSrc)
		.then((loadedImage) => {
			if (!alive) return;
			image = loadedImage;
			const size = workSize(image.naturalWidth, image.naturalHeight, PREVIEW);
			preview = pixelsAt(image, size.w, size.h);
			loaded = true;
			reorient();
		})
		.catch(() => (failed = true));
	return () => {
		alive = false;
		observer.disconnect();
		mq.removeEventListener('change', onChange);
		cancelCarve?.();
		clearTimeout(carveTimer);
		transition = null;
		effects.seam = null;
		effects.trail = [];
	};
});
</script>

<svelte:window onkeydown={keydown} />

{#snippet dockTitle()}
	<span class="sep"></span>
	{#if backHref}
		<a class="btn icon" href={backHref} aria-label="Back to the framer" title="Back to the framer">‹</a>
	{/if}
	<span class="muted">Carve{#if !phone} · {title}{/if}</span>
{/snippet}

{#snippet views()}
	<div class="seg views">
		{#each VIEWS as v, i (v)}
			<button class:on={view === v} title="{v} ({i + 1})" onclick={() => setView(v)}>{v[0].toUpperCase() + v.slice(1)}</button>
		{/each}
	</div>
{/snippet}

{#snippet actions()}
	<button class="btn" disabled={!loaded || carving || busy !== null} onclick={() => run('export')}>
		{busy === 'export' ? 'Exporting…' : 'Export'}
	</button>
	<button class="btn primary" disabled={!loaded || carving || busy !== null} onclick={() => run('save')}>
		{busy === 'save' ? 'Saving…' : phone ? 'Save' : 'Save to library'}
	</button>
{/snippet}

<!-- The dock has no room for the views and actions on a phone; they sit under the photo instead. -->
{#snippet dockIslands()}
	{#if !phone}
		<div class="island mid">{@render views()}</div>
		<div class="island end actions">{@render actions()}</div>
	{/if}
{/snippet}

<div class="carver">
	<section class="stage">
		<canvas
			class="view"
			bind:this={canvas}
			{@attach wheelListener}
			onpointerdown={pointerdown}
			onpointermove={pointermove}
			onpointerup={pointerup}
			onpointerleave={pointerleave}
		></canvas>
		<canvas class="marks" bind:this={marks}></canvas>
		<canvas class="fx" bind:this={fx}></canvas>
		{#if failed}
			<p class="muted">The photo could not be decoded</p>
		{/if}
		{#if phone}
			<div class="bar-row">{@render views()}</div>
		{/if}
		<div class="island progress">
			<div class="track"><div class="bar" style:width="{(100 * steps) / Math.max(maxSeams, 1)}%"></div></div>
			<span class="status">{status}</span>
			<button class="btn" disabled={carving || !loaded} onclick={replay}>Replay</button>
		</div>
		{#if phone}
			<div class="bar-row actions">{@render actions()}</div>
		{/if}
	</section>

	<aside class="panel">
		<fieldset>
			<legend>Shape</legend>
			<div class="field">
				<span>Direction</span>
				<div class="seg wide">
					<button class:on={settings.direction === 'columns'} onclick={() => setDirection('columns')}>Narrower</button>
					<button class:on={settings.direction === 'rows'} onclick={() => setDirection('rows')}>Shorter</button>
				</div>
			</div>
			<label class="field">
				<span>Size</span><output class:warn={settings.size < 0.6}>{sizeText}</output>
				<input type="range" min="40" max="140" bind:value={() => settings.size * 100, (v) => (settings.size = v / 100)} oninput={requestDraw} />
				<small>Instant once carved. Past 40% removed, anything unprotected starts to narrow.</small>
			</label>
		</fieldset>

		<fieldset class:selected={selected !== null}>
			<legend>{selected ? 'Selected point' : 'New points'}</legend>
			<div class="field">
				<span>Kind</span>
				<div class="seg wide">
					<button class:on={editing.kind === 'protect'} onclick={() => changePoint((p) => (p.kind = 'protect'))}>Protect</button>
					<button class:on={editing.kind === 'remove'} onclick={() => changePoint((p) => (p.kind = 'remove'))}>Remove</button>
				</div>
				<small>Protect keeps seams out of the disc. Remove sends the first seams through it, deleting what is there.</small>
			</div>
			<label class="field">
				<span>Size</span><output>{Math.round(editing.size * 100)}% of short side</output>
				<input type="range" min="1" max="40" value={Math.round(editing.size * 100)} oninput={(e) => changePoint((p) => (p.size = Number(e.currentTarget.value) / 100))} />
			</label>
			<label class="field">
				<span>Protection</span><output>{strengthText(editing.strength)}</output>
				<input type="range" min="0" max="10" value={editing.strength} oninput={(e) => changePoint((p) => (p.strength = Number(e.currentTarget.value)))} />
				<small>Soft lets a seam through when everything else costs more.</small>
			</label>
			<label class="check"><input type="checkbox" checked={showPoints} onchange={(e) => setShowPoints(e.currentTarget.checked)} /> Show points</label>
			<div class="row">
				{#if selected}
					<button class="btn" onclick={deleteSelected}>Delete point</button>
				{/if}
				<button class="btn" disabled={points.length === 0} onclick={clearPoints}>Clear all</button>
			</div>
			<small>
				{#if phone}
					Tap the photo to add a point and drag to move it. Tap a point to select it, then change it here.
				{:else if selected}
					Changes apply to this point. Esc goes back to the settings for new points.
				{:else}
					These settings apply to the next point you add. Click the photo to add one, ⌥ click for a remove point, scroll or [ ] for size.
				{/if}
			</small>
		</fieldset>

		<details>
			<summary>Advanced</summary>
			<fieldset>
				<legend>Region</legend>
				<label class="field">
					<span>{edgeWords[0]}</span><output>{Math.round(settings.from * 100)}% of the {edgeWords[2]}</output>
					<input type="range" min="0" max="100" bind:value={() => settings.from * 100, (v) => (settings.from = v / 100)} oninput={regionInput} />
				</label>
				<label class="field">
					<span>{edgeWords[1]}</span><output>{Math.round(settings.to * 100)}% of the {edgeWords[2]}</output>
					<input type="range" min="0" max="100" bind:value={() => settings.to * 100, (v) => (settings.to = v / 100)} oninput={regionInput} />
				</label>
				<small>Seams may only run between these two lines. Outside is dimmed on the photo.</small>
			</fieldset>
			<fieldset>
				<legend>Steering</legend>
				<label class="field">
					<span>Lean</span><output>{leanText}</output>
					<input type="range" min="-10" max="10" bind:value={settings.lean} oninput={shapeInput} />
					<small>Makes one side cheaper to cut, without forbidding the other.</small>
				</label>
				<label class="field">
					<span>Straightness</span><output>{settings.straight === 0 ? 'off' : settings.straight}</output>
					<input type="range" min="0" max="40" bind:value={settings.straight} oninput={shapeInput} />
					<small>Penalises seams for stepping sideways, so they wander less through lines.</small>
				</label>
			</fieldset>
			<fieldset>
				<legend>Detail measure</legend>
				<div class="field">
					<span>Energy</span>
					<div class="seg wide">
						<button class:on={settings.energy === 'forward'} onclick={() => setEnergy('forward')}>Forward</button>
						<button class:on={settings.energy === 'gradient'} onclick={() => setEnergy('gradient')}>Gradient</button>
					</div>
					<small>Forward avoids creating new edges; gradient avoids removing detail, as caire does. E switches.</small>
				</div>
				<label class="field">
					<span>Blur</span><output>{settings.blur === 0 ? 'off' : `${settings.blur} px`}</output>
					<input type="range" min="0" max="6" bind:value={settings.blur} oninput={shapeInput} />
					<small>Smooths fine texture before measuring.</small>
				</label>
				<label class="field">
					<span>Working size</span>
					<select bind:value={settings.working} onchange={scheduleCarve}>
						<option value={512}>512 px, fastest</option>
						<option value={768}>768 px</option>
						<option value={1024}>1024 px</option>
					</select>
					<small>The size the seams are computed at. The version gets them stretched over the full-size photo. Bigger is slower, with finer seams.</small>
				</label>
			</fieldset>
			<fieldset>
				<legend>Seam style</legend>
				<label class="field"><span>Colour</span><input type="color" bind:value={seamColour} oninput={requestDraw} /></label>
				<label class="check"><input type="checkbox" bind:checked={glow} /> Glow: flicker and afterglow</label>
				<label class="field">
					<span>Size</span><output>{fxSize}%</output>
					<input type="range" min="25" max="300" step="25" bind:value={fxSize} />
				</label>
			</fieldset>
		</details>
	</aside>
</div>

<style>
	.carver {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 300px;
		height: 100vh;
		background: var(--bg);
	}
	.stage {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: 0;
		padding: 64px 16px 60px;
	}
	.view {
		display: block;
		min-width: 0;
		max-width: 100%;
		max-height: calc(100vh - 124px);
		cursor: crosshair;
		touch-action: none;
	}
	.marks,
	.fx {
		position: absolute;
		pointer-events: none;
	}
	.stage > .muted {
		position: absolute;
		color: var(--subtle);
	}
	.muted {
		color: var(--subtle);
	}
	.progress {
		bottom: 14px;
		left: 16px;
		right: auto;
		gap: 12px;
		font-size: 12px;
		color: var(--subtle);
		font-variant-numeric: tabular-nums;
	}
	.track {
		flex: 0 0 120px;
		height: 4px;
		border-radius: 2px;
		background: #3f3f46;
		overflow: hidden;
	}
	.bar {
		height: 100%;
		background: var(--brand-hover);
	}
	.status {
		white-space: nowrap;
	}
	.island.mid {
		padding: 0 4px;
	}
	.actions {
		gap: 4px;
	}

	.panel {
		display: flex;
		flex-direction: column;
		gap: 12px;
		padding: 64px 16px 24px 0;
		overflow-y: auto;
		font-size: 13px;
	}
	fieldset {
		border: 1px solid var(--edge);
		border-radius: 10px;
		padding: 10px 12px 12px;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 9px;
	}
	fieldset.selected {
		border-color: rgba(96, 165, 250, 0.55);
	}
	legend {
		color: var(--subtle);
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.12em;
		padding: 0 4px;
	}
	fieldset.selected legend {
		color: #60a5fa;
	}
	details {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	summary {
		cursor: pointer;
		color: var(--subtle);
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.12em;
		padding: 4px;
		margin-bottom: 8px;
	}
	.field {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: 2px 8px;
		align-items: center;
	}
	.field > span {
		color: var(--fg);
	}
	.field output {
		color: var(--subtle);
		font-variant-numeric: tabular-nums;
		font-size: 12px;
	}
	.field output.warn {
		color: #fbbf24;
	}
	.field input[type='range'],
	.field select,
	.field .seg {
		grid-column: 1 / -1;
		width: 100%;
	}
	.seg.wide button {
		flex: 1;
	}
	.field input[type='color'] {
		width: 44px;
		height: 26px;
		padding: 0 2px;
		border: 1px solid var(--edge);
		border-radius: 6px;
		background: var(--surface-solid);
		cursor: pointer;
	}
	.field small,
	fieldset > small {
		grid-column: 1 / -1;
		color: var(--subtle);
		opacity: 0.8;
		font-size: 11.5px;
		line-height: 1.45;
	}
	select {
		background: var(--surface-solid);
		color: var(--fg);
		border: 1px solid var(--edge);
		border-radius: 6px;
		padding: 5px 6px;
		font: inherit;
	}
	input[type='range'] {
		accent-color: var(--brand-hover);
		margin: 2px 0;
	}
	.check {
		display: flex;
		gap: 8px;
		align-items: center;
		cursor: pointer;
	}
	.row {
		display: flex;
		gap: 8px;
	}
	.row .btn {
		flex: 1;
		border: 1px solid var(--edge);
	}

	/* Phones: the photo stays pinned at the top while the controls scroll under it. */
	@media (max-width: 700px) {
		.carver {
			display: block;
			height: auto;
			min-height: 100vh;
		}
		.stage {
			position: sticky;
			top: 0;
			z-index: 5;
			display: grid;
			justify-items: center;
			gap: 8px;
			padding: 66px 8px 8px;
			background: var(--bg);
			box-shadow: 0 8px 20px rgba(0, 0, 0, 0.6);
		}
		.view {
			max-height: 40svh;
		}
		.bar-row {
			display: flex;
			width: 100%;
			gap: 8px;
		}
		.bar-row .views,
		.bar-row .views button,
		.bar-row .btn {
			flex: 1;
		}
		.bar-row .views button {
			padding: 0;
		}
		.progress {
			position: static;
			width: 100%;
			min-width: 0;
		}
		.status {
			overflow: hidden;
			text-overflow: ellipsis;
		}
		.progress .btn {
			margin-left: auto;
		}
		.track {
			flex: 0 0 48px;
		}
		.panel {
			padding: 12px 12px 40px;
		}
		.row .btn {
			height: 36px;
		}
	}
</style>
