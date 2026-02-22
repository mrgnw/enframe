import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

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
	esbuild: false
});
