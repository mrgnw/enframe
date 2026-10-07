// A throwaway copy of the sample folder, so the tests never touch the user's own `samples/train clouds.pin.json`.
// Photos are hard links (no copy: the folder is 43 MB), which `listPhotos` accepts because `isFile()` is true
// for them; a symlink would fail that test. Hard links need one file system, so the folder lives under out/,
// inside the repository; where linking fails anyway (EXDEV), the photos are copied.
import { createHash } from 'node:crypto';
import { copyFile, link, mkdir, mkdtemp, readdir, readFile, rm } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
export const SAMPLES = join(ROOT, 'samples', 'train clouds');
export const BIN = resolve(ROOT, process.env.PINFRAME_BIN ?? 'rust/target/release/pinframe');
export const SHOTS = join(ROOT, 'out', 'e2e', 'shots');

/** The user's own document for the sample folder; its hash is checked before and after the run. */
const USER_DOC = `${SAMPLES}.pin.json`;

async function fingerprint(path: string): Promise<string> {
	try {
		return createHash('sha1').update(await readFile(path)).digest('hex');
	} catch {
		return 'absent';
	}
}

export interface Sandbox {
	/** `<work>/train clouds`: the linked photos. The app saves `<work>/train clouds.pin.json` beside it. */
	folder: string;
	doc: string;
	/** A directory beside the folder for exports and CLI runs. */
	out: string;
	linked: 'hard links' | 'copies';
	ms: number;
	/** Removes the work folder and fails when the user's own document changed. */
	dispose(): Promise<void>;
}

export async function sandbox(): Promise<Sandbox> {
	const started = performance.now();
	const before = await fingerprint(USER_DOC);
	await mkdir(join(ROOT, 'out'), { recursive: true });
	const work = await mkdtemp(join(ROOT, 'out', 'e2e-work-'));
	const folder = join(work, 'train clouds');
	const out = join(work, 'out');
	await mkdir(folder);
	await mkdir(out);
	let linked: Sandbox['linked'] = 'hard links';
	for (const entry of await readdir(SAMPLES, { withFileTypes: true })) {
		if (!entry.isFile()) continue;
		try {
			await link(join(SAMPLES, entry.name), join(folder, entry.name));
		} catch (e) {
			if ((e as NodeJS.ErrnoException).code !== 'EXDEV') throw e;
			linked = 'copies';
			await copyFile(join(SAMPLES, entry.name), join(folder, entry.name));
		}
	}
	return {
		folder,
		doc: `${folder}.pin.json`,
		out,
		linked,
		ms: performance.now() - started,
		async dispose() {
			if (!process.env.PINFRAME_E2E_KEEP) await rm(work, { recursive: true, force: true });
			const after = await fingerprint(USER_DOC);
			if (after !== before) throw new Error(`the tests changed ${USER_DOC}`);
		}
	};
}
