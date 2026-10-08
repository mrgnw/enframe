// The known marks of the train clouds job, read from analysis/train_clouds_all.sh so the browser tests and
// the baseline check share one source: the four reference points and one `--at` mark per lost photo.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { ROOT } from './sandbox.ts';

export type Point = [number, number];
export interface Marks {
	points: Point[];
	at: Record<string, { points: Point[]; zoom?: number }>;
	near?: Record<string, { points: Point[]; zoom?: number }>;
	skip?: string[];
}

const parsePoints = (text: string): Point[] =>
	text.split('/').map((p) => p.split(',').map(Number) as Point);

export function scriptMarks(): Marks {
	const script = readFileSync(join(ROOT, 'analysis', 'train_clouds_all.sh'), 'utf8');
	const points = /--points (\S+)/.exec(script)?.[1];
	if (!points) throw new Error('no --points in analysis/train_clouds_all.sh');
	const at: Marks['at'] = {};
	for (const m of script.matchAll(/^\s*--at (\S+\.avif)=([^@\s]+)(?:@([\d.]+))?$/gm)) {
		at[m[1]] = { points: parsePoints(m[2]), ...(m[3] && { zoom: Number(m[3]) }) };
	}
	if (Object.keys(at).length < 20) throw new Error('could not read the --at marks of train_clouds_all.sh');
	return { points: parsePoints(points), at };
}

/** The CLI flags for a document, written out here on purpose: not the app's own `cliArgs`. */
export function flags(marks: Marks): string[] {
	const list = (ps: Point[]) => ps.map(([x, y]) => `${x},${y}`).join('/');
	const args = ['--points', list(marks.points)];
	for (const [file, m] of Object.entries(marks.at)) {
		args.push('--at', `${file}=${list(m.points)}${m.zoom === undefined ? '' : `@${m.zoom}`}`);
	}
	for (const [file, m] of Object.entries(marks.near ?? {})) {
		args.push('--near', `${file}=${list(m.points)}${m.zoom === undefined ? '' : `@${m.zoom}`}`);
	}
	for (const file of marks.skip ?? []) args.push('--skip', file);
	return args;
}
