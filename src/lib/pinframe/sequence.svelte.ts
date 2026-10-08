// The folder's features: one marking session each, the one on screen, and the document they save together.
// Every track posts the whole document, so a feature's marks are saved whichever feature tracks next.

import { loadFolder } from './api';
import { Session } from './session.svelte';
import { type Feature, FIRST_FEATURE, type TrailMode } from './types';

/** Turns of the overlay's hue, one per feature in the order they were made; the first keeps the mockup's. */
const HUES = [0, 150, 240, 300, 60, 200];

export class Sequence {
	readonly folder: string;
	readonly trailMode: TrailMode;
	sessions = $state<Session[]>([]);
	active = $state(0);
	private files: string[] = [];
	private made = 0;

	constructor(folder: string, trailMode: TrailMode) {
		this.folder = folder;
		this.trailMode = trailMode;
		this.sessions = [this.make(FIRST_FEATURE)];
	}

	get session(): Session {
		return this.sessions[this.active];
	}

	/** The features with points, the document every track saves. */
	document(): Feature[] {
		return this.sessions.filter((s) => s.marks.points.length).map((s) => ({ name: s.name, marks: s.marks }));
	}

	private make(name: string): Session {
		return new Session(this.folder, this.trailMode, name, HUES[this.made++ % HUES.length], () => this.document());
	}

	async load(): Promise<void> {
		let features: Feature[];
		try {
			({ photos: this.files, features } = await loadFolder(this.folder));
		} catch (e) {
			this.session.stage = 'failed';
			this.session.error = (e as Error).message;
			return;
		}
		if (features.length) {
			this.made = 0;
			this.sessions = features.map((f) => this.make(f.name));
		}
		this.sessions.forEach((s, i) => {
			s.init(this.files, features[i]?.marks ?? null);
		});
		this.session.show();
	}

	select(i: number): void {
		if (i === this.active || !this.sessions[i]) return;
		this.active = i;
		this.session.show();
	}

	cycle(by: number): void {
		const n = this.sessions.length;
		this.select((((this.active + by) % n) + n) % n);
	}

	/** A new feature starts at the Points step on the first photo. */
	add(): void {
		const taken = new Set(this.sessions.map((s) => s.name));
		let n = this.sessions.length + 1;
		while (taken.has(`Feature ${n}`)) n++;
		const session = this.make(`Feature ${n}`);
		session.init(this.files, null);
		this.sessions.push(session);
		this.select(this.sessions.length - 1);
	}

	/** Why `name` cannot name the feature on screen, or null when it can. */
	refusal(name: string): string | null {
		if (!name.trim()) return 'A feature needs a name';
		if (name.includes('/')) return 'A name cannot hold a slash';
		if (this.sessions.some((s) => s !== this.session && s.name === name)) return `${name} is taken`;
		return null;
	}

	rename(name: string): void {
		if (this.refusal(name)) return;
		this.session.name = name;
		this.save();
	}

	/** Another feature must have points, or the document would have nothing left to save. */
	get canRemove(): boolean {
		return this.sessions.some((s) => s !== this.session && s.marks.points.length);
	}

	remove(): void {
		if (!this.canRemove) return;
		this.sessions.splice(this.active, 1);
		this.active = Math.min(this.active, this.sessions.length - 1);
		this.save();
	}

	// ponytail: saving a rename or a removal re-tracks a feature (about 0.2 s); a save-only route if that shows.
	private save(): void {
		const saver = this.session.marks.points.length ? this.session : this.sessions.find((s) => s.marks.points.length);
		void saver?.retrack();
	}
}
