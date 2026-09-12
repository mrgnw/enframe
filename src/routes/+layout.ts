import { resolveConnection } from '$lib/bridge';

export const prerender = true;
export const ssr = false;

// Resolved before anything renders, so every path-shaped helper can stay synchronous
// about which instance owns the data.
export async function load() {
	await resolveConnection();
}
