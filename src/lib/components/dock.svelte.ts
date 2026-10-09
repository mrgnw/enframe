// The top dock is both apps' chrome: each app's layout draws it on every screen and in every state, with the
// app switch first. A screen adds its title after the switch and its own islands with `useDock`.

import { getContext, onDestroy, setContext, type Snippet } from 'svelte';

const KEY = Symbol('dock');

export interface DockParts {
	/** After the switch in the first island: what the screen is on, with its leading separator. */
	title?: Snippet;
	/** More islands in the dock, each placed with its `mid` or `end` class. */
	islands?: Snippet;
}

export class Dock {
	parts = $state.raw<DockParts>({});
	owner: symbol | null = null;
}

export const provideDock = (): Dock => setContext(KEY, new Dock());

/**
 * Shows `parts` in the dock while the calling component is mounted; call it during component init. The
 * snippets read the screen's own state, so they stay current without being handed over again.
 */
export function useDock(parts: DockParts): void {
	const dock = getContext<Dock | undefined>(KEY);
	if (!dock) throw new Error('useDock needs an app layout: src/routes/(enframe) or src/routes/stabilize');
	const me = Symbol('dock owner');
	dock.owner = me;
	dock.parts = parts;
	onDestroy(() => {
		// The next screen may have taken the dock before this one is torn down.
		if (dock.owner !== me) return;
		dock.owner = null;
		dock.parts = {};
	});
}
