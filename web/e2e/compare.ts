// Sidecar comparison: the exported clip's crop rects against a CLI run's.
import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { BIN, ROOT } from './sandbox.ts';
import { flags, type Marks } from './marks.ts';

interface Row {
	file: string;
	x: number;
	y: number;
	w: number;
	h: number;
	scale: number;
	source: string;
	points: [number, number][];
}

export const readSidecar = (path: string): Row[] => JSON.parse(readFileSync(path, 'utf8')).photos;

/** The tracker's full-precision doubles survive the JSON round trip, so a rerun gives 0; this is float noise. */
const TOLERANCE = 1e-9;

/** The worst difference between two sidecars' rects, scales and anchor points; throws on a differing name or source. */
export function worstDifference(a: Row[], b: Row[]): number {
	if (a.length !== b.length) throw new Error(`${a.length} rows against ${b.length}`);
	let worst = 0;
	a.forEach((row, i) => {
		const other = b[i];
		if (row.file !== other.file) throw new Error(`row ${i}: ${row.file} against ${other.file}`);
		if (row.source !== other.source) throw new Error(`${row.file}: ${row.source} against ${other.source}`);
		const pairs = [
			...(['x', 'y', 'w', 'h', 'scale'] as const).map((k) => [row[k], other[k]]),
			...row.points.flatMap((p, j) => [
				[p[0], other.points[j][0]],
				[p[1], other.points[j][1]]
			])
		];
		for (const [p, q] of pairs) worst = Math.max(worst, Math.abs(p - q));
	});
	if (worst > TOLERANCE) throw new Error(`sidecars differ by ${worst}`);
	return worst;
}

/** A JSON-only CLI run of a folder with a document's marks and transparent padding (the preview's default). */
export function cliSidecar(folder: string, marks: Marks, out: string): Row[] {
	const photos = readdirSync(folder)
		.filter((f) => f.endsWith('.avif'))
		.sort()
		.map((f) => join(folder, f));
	execFileSync(BIN, [...photos, ...flags(marks), '--pad', 'transparent', '--out', out], { cwd: ROOT, stdio: 'pipe' });
	return readSidecar(out);
}

/** analysis/train_clouds_all.sh itself, JSON-only. It reads the committed samples folder and writes only `out`. */
export function scriptSidecar(out: string): Row[] {
	execFileSync('bash', [join(ROOT, 'analysis', 'train_clouds_all.sh'), out], {
		cwd: ROOT,
		stdio: 'pipe',
		env: { ...process.env, BIN }
	});
	return readSidecar(out);
}
