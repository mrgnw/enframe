// Composition scoring engine for auto dual-frame placement.
// Scores frame placements based on how well focal points align with
// classical compositional power points (rule of thirds, phi grid, etc.)

export interface Point {
	x: number;
	y: number;
}

export interface FocalInput extends Point {
	required?: boolean;
}

export interface Rect {
	x: number;
	y: number;
	width: number;
	height: number;
}

export type CompositionSystem = 'thirds' | 'phi' | 'spiral' | 'diagonal' | 'center';

export interface CompositionMatch {
	system: CompositionSystem;
	proximity: number; // 0-1, higher = closer to power point
}

export interface ScoredPlacement {
	rect: Rect;
	score: number;
	matches: CompositionMatch[][]; // per focal point
	label: string; // e.g. "Phi + Thirds"
	satisfiesRequired: boolean;
}

// Power points for each compositional system (frame-relative 0-1 coords)

const THIRDS_POINTS: Point[] = [
	{ x: 1 / 3, y: 1 / 3 },
	{ x: 2 / 3, y: 1 / 3 },
	{ x: 1 / 3, y: 2 / 3 },
	{ x: 2 / 3, y: 2 / 3 }
];

const PHI = 0.618033988749895;
const PHI_INV = 1 - PHI; // ~0.382

const PHI_POINTS: Point[] = [
	{ x: PHI_INV, y: PHI_INV },
	{ x: PHI, y: PHI_INV },
	{ x: PHI_INV, y: PHI },
	{ x: PHI, y: PHI }
];

// Golden spiral eye positions (4 rotations, one per corner)
const SPIRAL_POINTS: Point[] = [
	{ x: PHI_INV, y: PHI_INV }, // top-left eye
	{ x: PHI, y: PHI_INV }, // top-right eye
	{ x: PHI_INV, y: PHI }, // bottom-left eye
	{ x: PHI, y: PHI } // bottom-right eye
];

const CENTER_POINTS: Point[] = [{ x: 0.5, y: 0.5 }];

// Diagonal method: 45-degree lines from corners of the inscribed square.
// For a frame with aspect ratio w:h, the inscribed square sits centered.
// We sample points along the 45-degree diagonals from each corner of that square.
function getDiagonalPoints(frameAspect: number): Point[] {
	const points: Point[] = [];

	if (frameAspect >= 1) {
		// Landscape: square is h×h centered horizontally
		const squareW = 1 / frameAspect; // normalized width of inscribed square
		const offsetX = (1 - squareW) / 2;
		// 4 diagonal lines from corners of inscribed square
		// Sample at key intersections
		for (let t = 0.2; t <= 0.8; t += 0.2) {
			points.push({ x: offsetX + squareW * t, y: t }); // top-left to bottom-right
			points.push({ x: offsetX + squareW * (1 - t), y: t }); // top-right to bottom-left
		}
	} else {
		// Portrait: square is w×w centered vertically
		const squareH = frameAspect; // normalized height of inscribed square
		const offsetY = (1 - squareH) / 2;
		for (let t = 0.2; t <= 0.8; t += 0.2) {
			points.push({ x: t, y: offsetY + squareH * t });
			points.push({ x: 1 - t, y: offsetY + squareH * t });
		}
	}

	return points;
}

interface SystemWeights {
	thirds: number;
	phi: number;
	spiral: number;
	diagonal: number;
	center: number;
}

const LANDSCAPE_WEIGHTS: SystemWeights = {
	phi: 1.0,
	thirds: 0.9,
	spiral: 0.7,
	center: 0.4,
	diagonal: 0.6
};

const PORTRAIT_WEIGHTS: SystemWeights = {
	phi: 0.8,
	thirds: 0.6,
	spiral: 1.0,
	center: 0.7,
	diagonal: 0.5
};

const PROXIMITY_SENSITIVITY = 8; // higher = more forgiving distance
const CONSENSUS_THRESHOLD = 0.4; // min proximity to count as "matching"
const CONSENSUS_BONUS = 1.3;

const COMPOSITION_WEIGHT = 1.0;
const CROP_WEIGHT = 0.5;
const ZOOM_WEIGHT = 0.8;
const ZOOM_COMFORT_SCALE = 0.55;
const MIN_SCALE = 0.45;
const SCALE_STEPS = 6;
const POSITION_STEPS = 24;

function distance(a: Point, b: Point): number {
	const dx = a.x - b.x;
	const dy = a.y - b.y;
	return Math.sqrt(dx * dx + dy * dy);
}

function clamp01(value: number): number {
	return Math.max(0, Math.min(1, value));
}

export function containsPoint(rect: Rect, point: Point): boolean {
	return point.x >= rect.x && point.x <= rect.x + rect.width && point.y >= rect.y && point.y <= rect.y + rect.height;
}

function missDistance(rect: Rect, points: Point[]): number {
	let total = 0;
	for (const p of points) {
		const dx = Math.max(rect.x - p.x, 0, p.x - (rect.x + rect.width));
		const dy = Math.max(rect.y - p.y, 0, p.y - (rect.y + rect.height));
		total += Math.sqrt(dx * dx + dy * dy);
	}
	return total;
}

export function excludedRequiredPoints(rect: Rect, points: FocalInput[]): FocalInput[] {
	return points.filter((p) => p.required && !containsPoint(rect, p));
}

// A point applies to a frame when it is global (no frame_id) or bound to that frame.
export function clipsRequiredPoints(
	rect: Rect,
	frameId: number,
	points: { x: number; y: number; required: boolean; frame_id?: number | null }[]
): boolean {
	return points.some((p) => p.required && (!p.frame_id || p.frame_id === frameId) && !containsPoint(rect, p));
}

// Position for CSS `object-fit: cover` when the container matches the crop ratio.
export function objectPosition(rect: Rect): string {
	const px = rect.width >= 1 ? 0.5 : clamp01(rect.x / (1 - rect.width));
	const py = rect.height >= 1 ? 0.5 : clamp01(rect.y / (1 - rect.height));
	return `${(px * 100).toFixed(1)}% ${(py * 100).toFixed(1)}%`;
}

function getSystemPowerPoints(system: CompositionSystem, frameAspect: number): Point[] {
	switch (system) {
		case 'thirds':
			return THIRDS_POINTS;
		case 'phi':
			return PHI_POINTS;
		case 'spiral':
			return SPIRAL_POINTS;
		case 'diagonal':
			return getDiagonalPoints(frameAspect);
		case 'center':
			return CENTER_POINTS;
	}
}

const ALL_SYSTEMS: CompositionSystem[] = ['thirds', 'phi', 'spiral', 'diagonal', 'center'];

function scoreCandidate(
	candidate: Rect,
	focalPoints: Point[],
	weights: SystemWeights,
	frameAspect: number
): { score: number; matches: CompositionMatch[][] } {
	let totalScore = 0;
	const allMatches: CompositionMatch[][] = [];

	for (const fp of focalPoints) {
		// Transform to frame-relative coords
		const rel: Point = {
			x: (fp.x - candidate.x) / candidate.width,
			y: (fp.y - candidate.y) / candidate.height
		};

		// Skip if point is outside frame (shouldn't happen with proper constraints)
		if (rel.x < 0 || rel.x > 1 || rel.y < 0 || rel.y > 1) {
			allMatches.push([]);
			continue;
		}

		let pointScore = 0;
		let systemsMatched = 0;
		const pointMatches: CompositionMatch[] = [];

		for (const system of ALL_SYSTEMS) {
			const powerPoints = getSystemPowerPoints(system, frameAspect);
			let minDist = Infinity;
			for (const pp of powerPoints) {
				const d = distance(rel, pp);
				if (d < minDist) minDist = d;
			}

			const proximity = Math.max(0, 1 - minDist * PROXIMITY_SENSITIVITY);
			const weight = weights[system];
			pointScore += proximity * weight;

			if (proximity > CONSENSUS_THRESHOLD) {
				systemsMatched++;
				pointMatches.push({ system, proximity });
			}
		}

		if (systemsMatched >= 2) {
			pointScore *= CONSENSUS_BONUS;
		}

		totalScore += pointScore;
		allMatches.push(pointMatches);
	}

	return { score: totalScore, matches: allMatches };
}

function buildLabel(matches: CompositionMatch[][]): string {
	// Aggregate which systems appear most across all focal points
	const systemCounts = new Map<CompositionSystem, number>();
	const systemProximity = new Map<CompositionSystem, number>();

	for (const pointMatches of matches) {
		for (const m of pointMatches) {
			systemCounts.set(m.system, (systemCounts.get(m.system) ?? 0) + 1);
			systemProximity.set(m.system, Math.max(systemProximity.get(m.system) ?? 0, m.proximity));
		}
	}

	const SYSTEM_LABELS: Record<CompositionSystem, string> = {
		thirds: 'Thirds',
		phi: 'Phi',
		spiral: 'Spiral',
		diagonal: 'Diagonal',
		center: 'Center'
	};

	// Sort by count then proximity, take top 2
	const sorted = [...systemCounts.entries()]
		.sort((a, b) => {
			if (b[1] !== a[1]) return b[1] - a[1];
			return (systemProximity.get(b[0]) ?? 0) - (systemProximity.get(a[0]) ?? 0);
		})
		.slice(0, 2);

	if (sorted.length === 0) return '';
	return sorted.map(([sys]) => SYSTEM_LABELS[sys]).join(' + ');
}

// Largest rect of the target ratio that fits the image, in normalized image coords.
// If image is 4000x3000 (aspect 4/3) and the target is 16:9, the frame's normalized
// width/height ratio is targetAspect / imageAspect.
function maxRect(normalizedAspect: number): { width: number; height: number } {
	if (normalizedAspect >= 1) {
		return { width: 1, height: 1 / normalizedAspect };
	}
	return { width: normalizedAspect, height: 1 };
}

// ponytail: for a fixed ratio both crop and zoom derive from one scale variable, so the
// two penalties only differ in curve shape (linear pull vs. knee below comfort scale).
// Splitting them properly needs per-destination output pixel budgets.
export function computeAutoFrame(
	focalPoints: FocalInput[],
	targetAspect: number, // w/h ratio, e.g. 16/9 for landscape
	imageAspect: number // w/h ratio of the source image
): ScoredPlacement {
	const max = maxRect(targetAspect / imageAspect);
	const centered: Rect = {
		x: (1 - max.width) / 2,
		y: (1 - max.height) / 2,
		width: max.width,
		height: max.height
	};

	if (focalPoints.length === 0) {
		return { rect: centered, score: 0, matches: [], label: '', satisfiesRequired: true };
	}

	const weights = targetAspect > 1 ? LANDSCAPE_WEIGHTS : PORTRAIT_WEIGHTS;
	const weightSum = ALL_SYSTEMS.reduce((sum, system) => sum + weights[system], 0);
	const required = focalPoints.filter((p) => p.required);

	let bestRect: Rect | null = null;
	let bestScore = -Infinity;
	let bestMatches: CompositionMatch[][] = [];
	let closestRect: Rect | null = null;
	let closestMiss = Infinity;

	for (let iScale = 0; iScale <= SCALE_STEPS; iScale++) {
		const scale = 1 - (iScale / SCALE_STEPS) * (1 - MIN_SCALE);
		const width = max.width * scale;
		const height = max.height * scale;
		const rangeX = 1 - width;
		const rangeY = 1 - height;
		const stepsX = rangeX > 0 ? POSITION_STEPS : 0;
		const stepsY = rangeY > 0 ? POSITION_STEPS : 0;

		const zoomMiss = Math.max(0, ZOOM_COMFORT_SCALE - scale) / ZOOM_COMFORT_SCALE;
		const sizeScore = CROP_WEIGHT * width * height - ZOOM_WEIGHT * zoomMiss * zoomMiss;

		for (let ix = 0; ix <= stepsX; ix++) {
			const x = stepsX > 0 ? (ix / stepsX) * rangeX : 0;
			for (let iy = 0; iy <= stepsY; iy++) {
				const y = stepsY > 0 ? (iy / stepsY) * rangeY : 0;
				const candidate: Rect = { x, y, width, height };

				const miss = missDistance(candidate, required);
				if (miss > 0) {
					if (miss < closestMiss) {
						closestMiss = miss;
						closestRect = candidate;
					}
					continue;
				}

				const { score, matches } = scoreCandidate(candidate, focalPoints, weights, targetAspect);
				const composition = score / (focalPoints.length * weightSum);
				const total = COMPOSITION_WEIGHT * composition + sizeScore;

				if (total > bestScore) {
					bestScore = total;
					bestRect = candidate;
					bestMatches = matches;
				}
			}
		}
	}

	if (!bestRect) {
		bestRect = closestRect ?? centered;
		const result = scoreCandidate(bestRect, focalPoints, weights, targetAspect);
		bestMatches = result.matches;
		bestScore = result.score;
	}

	return {
		rect: bestRect,
		score: bestScore,
		matches: bestMatches,
		label: buildLabel(bestMatches),
		satisfiesRequired: missDistance(bestRect, required) === 0
	};
}

export function computeFrameLabel(frame: Rect, focalPoints: Point[], targetAspect: number): string {
	if (focalPoints.length === 0) return '';
	const isLandscape = targetAspect > 1;
	const weights = isLandscape ? LANDSCAPE_WEIGHTS : PORTRAIT_WEIGHTS;
	const { matches } = scoreCandidate(frame, focalPoints, weights, targetAspect);
	return buildLabel(matches);
}

export const AUTO_LANDSCAPE_LABEL = 'Auto L';
export const AUTO_PORTRAIT_LABEL = 'Auto P';

export function isLegacyAutoLabel(label: string | undefined | null): boolean {
	return label === AUTO_LANDSCAPE_LABEL || label === AUTO_PORTRAIT_LABEL;
}

export function legacyAutoAspect(label: string | undefined | null): number | null {
	if (label === AUTO_LANDSCAPE_LABEL) return 16 / 9;
	if (label === AUTO_PORTRAIT_LABEL) return 9 / 16;
	return null;
}
