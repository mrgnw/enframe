// Composition scoring engine for auto dual-frame placement.
// Scores frame placements based on how well focal points align with
// classical compositional power points (rule of thirds, phi grid, etc.)

export interface Point {
	x: number;
	y: number;
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
const INSET_PADDING = 0.05; // 5% inset from frame edges for focal points
const MIN_FRAME_SIZE = 0.3; // minimum 30% of image dimension
const SEARCH_STEP = 0.01; // 1% increments for position search

function distance(a: Point, b: Point): number {
	const dx = a.x - b.x;
	const dy = a.y - b.y;
	return Math.sqrt(dx * dx + dy * dy);
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

export function computeAutoFrame(
	focalPoints: Point[],
	targetAspect: number, // w/h ratio, e.g. 16/9 for landscape
	imageAspect: number // w/h ratio of the source image
): ScoredPlacement | null {
	if (focalPoints.length === 0) return null;

	const isLandscape = targetAspect > 1;
	const weights = isLandscape ? LANDSCAPE_WEIGHTS : PORTRAIT_WEIGHTS;

	// All coords are normalized 0-1 in image space.
	// Frame dimensions in normalized coords depend on aspect ratios.
	// If image is 4000x3000 (aspect 4/3) and we want a 16:9 frame:
	//   frame pixel aspect = 16/9
	//   frame normalized width W, normalized height H
	//   (W * imgW) / (H * imgH) = targetAspect
	//   W / H = targetAspect / imageAspect
	const normalizedAspect = targetAspect / imageAspect;

	// Find bounding box of focal points
	let minX = Infinity,
		minY = Infinity,
		maxX = -Infinity,
		maxY = -Infinity;
	for (const fp of focalPoints) {
		minX = Math.min(minX, fp.x);
		minY = Math.min(minY, fp.y);
		maxX = Math.max(maxX, fp.x);
		maxY = Math.max(maxY, fp.y);
	}

	// Determine frame size: largest possible that contains all points with padding
	// Start with max size and shrink if needed
	let frameW: number, frameH: number;

	if (normalizedAspect >= 1) {
		// Frame is wider than tall in normalized space
		frameW = 1.0; // start at full image width
		frameH = frameW / normalizedAspect;
		if (frameH > 1.0) {
			frameH = 1.0;
			frameW = frameH * normalizedAspect;
		}
	} else {
		frameH = 1.0;
		frameW = frameH * normalizedAspect;
		if (frameW > 1.0) {
			frameW = 1.0;
			frameH = frameW / normalizedAspect;
		}
	}

	// Ensure frame is large enough to contain all points with padding
	const spreadX = maxX - minX;
	const spreadY = maxY - minY;
	const neededW = spreadX / (1 - 2 * INSET_PADDING);
	const neededH = spreadY / (1 - 2 * INSET_PADDING);

	if (neededW > frameW || neededH > frameH) {
		// Need to size frame to fit all points
		if (neededW / neededH > normalizedAspect) {
			// Width-constrained
			frameW = Math.min(1.0, neededW);
			frameH = frameW / normalizedAspect;
		} else {
			// Height-constrained
			frameH = Math.min(1.0, neededH);
			frameW = frameH * normalizedAspect;
		}
		// Clamp
		if (frameW > 1.0) {
			frameW = 1.0;
			frameH = frameW / normalizedAspect;
		}
		if (frameH > 1.0) {
			frameH = 1.0;
			frameW = frameH * normalizedAspect;
		}
	}

	// Enforce minimum size
	if (frameW < MIN_FRAME_SIZE) {
		frameW = Math.min(1.0, MIN_FRAME_SIZE);
		frameH = frameW / normalizedAspect;
	}
	if (frameH < MIN_FRAME_SIZE) {
		frameH = Math.min(1.0, MIN_FRAME_SIZE);
		frameW = frameH * normalizedAspect;
	}

	// Search for best position
	const rangeX = 1.0 - frameW;
	const rangeY = 1.0 - frameH;

	let bestScore = -Infinity;
	let bestRect: Rect | null = null;
	let bestMatches: CompositionMatch[][] = [];

	const stepsX = Math.max(1, Math.round(rangeX / SEARCH_STEP));
	const stepsY = Math.max(1, Math.round(rangeY / SEARCH_STEP));

	for (let ix = 0; ix <= stepsX; ix++) {
		const x = rangeX > 0 ? (ix / stepsX) * rangeX : 0;
		for (let iy = 0; iy <= stepsY; iy++) {
			const y = rangeY > 0 ? (iy / stepsY) * rangeY : 0;

			const candidate: Rect = { x, y, width: frameW, height: frameH };

			// Check all focal points are within padded region
			let allInside = true;
			for (const fp of focalPoints) {
				const relX = (fp.x - x) / frameW;
				const relY = (fp.y - y) / frameH;
				if (relX < INSET_PADDING || relX > 1 - INSET_PADDING || relY < INSET_PADDING || relY > 1 - INSET_PADDING) {
					allInside = false;
					break;
				}
			}
			if (!allInside) continue;

			const { score, matches } = scoreCandidate(candidate, focalPoints, weights, targetAspect);

			if (score > bestScore) {
				bestScore = score;
				bestRect = candidate;
				bestMatches = matches;
			}
		}
	}

	if (!bestRect) {
		// Fallback: center frame on centroid of focal points
		const cx = focalPoints.reduce((s, p) => s + p.x, 0) / focalPoints.length;
		const cy = focalPoints.reduce((s, p) => s + p.y, 0) / focalPoints.length;
		bestRect = {
			x: Math.max(0, Math.min(1 - frameW, cx - frameW / 2)),
			y: Math.max(0, Math.min(1 - frameH, cy - frameH / 2)),
			width: frameW,
			height: frameH
		};
		const result = scoreCandidate(bestRect, focalPoints, weights, targetAspect);
		bestMatches = result.matches;
		bestScore = result.score;
	}

	return {
		rect: bestRect,
		score: bestScore,
		matches: bestMatches,
		label: buildLabel(bestMatches)
	};
}

export function computeFrameLabel(
	frame: Rect,
	focalPoints: Point[],
	targetAspect: number,
	imageAspect: number
): string {
	if (focalPoints.length === 0) return '';
	const isLandscape = targetAspect > 1;
	const weights = isLandscape ? LANDSCAPE_WEIGHTS : PORTRAIT_WEIGHTS;
	const { matches } = scoreCandidate(frame, focalPoints, weights, targetAspect);
	return buildLabel(matches);
}

export const AUTO_LANDSCAPE_LABEL = 'Auto L';
export const AUTO_PORTRAIT_LABEL = 'Auto P';
export const AUTO_LABELS = [AUTO_LANDSCAPE_LABEL, AUTO_PORTRAIT_LABEL] as const;

export function isAutoFrame(label: string | undefined | null): boolean {
	return label === AUTO_LANDSCAPE_LABEL || label === AUTO_PORTRAIT_LABEL;
}
