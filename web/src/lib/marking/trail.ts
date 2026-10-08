// The overlay on the photo, as SVG markup: the current photo's points, the ghost at the cursor, and the
// trail, the feature's path through the neighbouring photos. Ported from mockups/01-marking.html as pure
// functions, so the same drawing ports into enframe at E3.

import { centre, seen, TRAIL_DEFAULT, type Photo, type Point, type TrailMode } from './types';

export const pointsSvg = (pts: Point[], cls: string, W: number, H: number) =>
	pts
		.map(
			([x, y]) =>
				`<circle class="pt ${cls}" cx="${x * W}" cy="${y * H}" r="${cls.startsWith('mark') || cls.startsWith('soft') ? 5.5 : cls === 'tracked' ? 4.5 : 6}"/>`
		)
		.join('');

/** The last photo before `i` where the feature was seen: the reference for marking `i`. */
export function prevLinked(photos: Photo[], i: number): Photo {
	for (let j = i - 1; j >= 0; j--) if (seen(photos[j])) return photos[j];
	return photos[0];
}

/**
 * The points the tracker would look for in `p` at `zoom`: the reference photo's points around their
 * centre, rescaled, as offsets in `p`'s 0–1 units.
 */
export function ghostOffsets(p: Photo, ref: Photo, zoom: number): Point[] {
	const [cx, cy] = centre(ref);
	const f = zoom / ref.refScale;
	return ref.pts.map(([x, y]) => [((x - cx) * ref.w * f) / p.w, ((y - cy) * ref.h * f) / p.h]);
}

const FLOW_MS = 1600;
const RAMP = {
	before: { near: [0.8, 0.14, 295], far: [0.72, 0.17, 352] },
	after: { near: [0.82, 0.13, 182], far: [0.78, 0.17, 145] }
};
type Side = keyof typeof RAMP;

interface Stop {
	q: Photo;
	k: number;
	x: number;
	y: number;
}

/**
 * The trail: a path through the points' centre in up to `trailN` photos each side, in order, with a dot
 * per photo, then the current photo's own points on top. `mode` shows distance from the current photo as
 * opacity (`fade`), one OKLCH colour per photo (`steps`, the default) or colours flowing along the path
 * (`flow`, a blurred glow sliding along gradient-blended segments, still under reduced motion).
 */
export function trailSvg(
	photos: Photo[],
	current: number,
	W: number,
	H: number,
	trailN: number,
	mode: TrailMode = 'steps',
	just = false
): string {
	const p = photos[current];
	const own = pointsSvg(p.pts, `${p.source}${just ? ' just' : ''}`, W, H);
	if (!trailN) return own;
	const spread = (d: number) => (trailN === 1 ? 0 : (d - 1) / (trailN - 1));
	const fade = (d: number) => (mode === 'fade' ? 1 - 0.45 * spread(d) : 1);
	const stepColour = (side: Side, d: number, lift = 0) => {
		const { near, far } = RAMP[side];
		const t = spread(Math.max(1, d));
		const [l, c, h] = near.map((v, i) => v + (far[i] - v) * t);
		return mode === 'fade' ? `var(--${side})` : `oklch(${(l + lift).toFixed(3)} ${c.toFixed(3)} ${h.toFixed(1)})`;
	};
	const flowGradient = (id: string, a: Stop, b: Stop, side: Side, lift: number) =>
		`<linearGradient id="${id}" gradientUnits="userSpaceOnUse" x1="${a.x}" y1="${a.y}" x2="${b.x}" y2="${b.y}">` +
		`<stop offset="0" stop-color="${stepColour(side, Math.abs(a.k), lift)}"/><stop offset="1" stop-color="${stepColour(side, Math.abs(b.k), lift)}"/></linearGradient>`;

	const cs: Stop[] = [];
	// A skipped photo gets no point: its position is only an estimate, so the line joins its neighbours directly.
	for (let k = -trailN; k <= trailN; k++) {
		const q = photos[current + k];
		if (q && q.pts.length && (k === 0 || q.source !== 'skipped')) {
			const [x, y] = centre(q);
			cs.push({ q, k, x: x * W, y: y * H });
		}
	}
	let lines = '';
	let dots = '';
	let flows = '';
	let defs = '';
	let run = 0;
	const delay = typeof performance === 'undefined' ? 0 : -(performance.now() % FLOW_MS);
	for (let j = 0; j + 1 < cs.length; j++) {
		const a = cs[j];
		const b = cs[j + 1];
		const side: Side = b.k <= 0 ? 'before' : 'after';
		const d = Math.max(Math.abs(a.k), Math.abs(b.k));
		const dash = a.q.source === 'lost' || b.q.source === 'lost' ? ' dashed' : '';
		const seg = `M${a.x} ${a.y}L${b.x} ${b.y}`;
		let stroke = stepColour(side, d);
		if (mode === 'flow') {
			defs += flowGradient(`fg${j}`, a, b, side, -0.2) + flowGradient(`fl${j}`, a, b, side, 0.22);
			stroke = `url(#fg${j})`;
			flows += `<path class="flow" d="${seg}" style="stroke:url(#fl${j});--o:${run.toFixed(1)}px;animation-delay:${delay.toFixed(0)}ms"/>`;
			run += Math.hypot(b.x - a.x, b.y - a.y);
		}
		lines += `<g opacity="${fade(d).toFixed(2)}"><path class="line halo" d="${seg}"/><path class="line ${side}${dash}" d="${seg}" style="stroke:${stroke}"/></g>`;
	}
	for (const { q, k, x, y } of cs) {
		if (k === 0) continue;
		const side: Side = k < 0 ? 'before' : 'after';
		const d = Math.abs(k);
		const r = q.source === 'mark' || q.source === 'soft' ? 5.5 : 4;
		dots += `<g opacity="${fade(d).toFixed(2)}"><circle class="dot-halo" cx="${x}" cy="${y}" r="${r + 2.5}"/><circle class="dot ${side} ${q.source}" cx="${x}" cy="${y}" r="${r}" style="color:${stepColour(side, d)}${q.source === 'tracked' ? '' : ';stroke:currentColor'}"/></g>`;
	}
	return `<defs>${defs}</defs>${lines}${flows}${dots}${own}`;
}

/** The reference patch: where the photo's background sits so its points' centre is in the middle. */
export function patchLayout(ref: Photo, pw: number, ph: number, patchPx: number) {
	const k = pw / (patchPx * ref.refScale);
	const [cx, cy] = centre(ref);
	const ox = pw / 2 - cx * ref.w * k;
	const oy = ph / 2 - cy * ref.h * k;
	return {
		size: `${ref.w * k}px auto`,
		position: `${ox}px ${oy}px`,
		points: ref.pts
			.map(([x, y]) => `<circle class="pt mark" cx="${ox + x * ref.w * k}" cy="${oy + y * ref.h * k}" r="3"/>`)
			.join('')
	};
}

export const trailModeFrom = (value: string | null): TrailMode =>
	(['fade', 'steps', 'flow'] as const).find((m) => m === value) ?? 'steps';

export { TRAIL_DEFAULT };
