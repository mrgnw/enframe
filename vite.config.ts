import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	server: {
		host: true,
		port: 5910,
		allowedHosts: true
	},
	experimental: {
		enableNativePlugin: true
	},
	esbuild: false,
	// Vitest runs test files through this same pipeline, so the svelte plugin compiles
	// `.svelte.ts` modules and runes work in tests against the real code.
	test: {
		include: ['src/**/*.{test,spec}.{js,ts}']
	},
	resolve: process.env.VITEST ? { conditions: ['browser'] } : undefined
});
