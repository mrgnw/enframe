import adapter from '@sveltejs/adapter-node';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

// The MVP's fixed port: `pnpm dev` and `pnpm start` both listen on it, on every interface, so a tailnet
// host (*.example.ts.net) reaches the app at http://<host>:7341/.
const port = Number(process.env.PORT ?? 7341);

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter()
		})
	],
	server: { port, host: true, strictPort: true },
	preview: { port, host: true, strictPort: true }
});
