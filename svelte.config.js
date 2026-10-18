import adapter from '@sveltejs/adapter-static';

const web = process.env.ENFRAME_TARGET === 'web';
const outDir = web ? 'build-web' : 'build';

/** @type {import('@sveltejs/kit').Config} */
export default {
	compilerOptions: {
		runes: true
	},
	kit: {
		adapter: adapter({
			pages: outDir,
			assets: outDir,
			fallback: 'index.html'
		}),
		// Registered by hand in the root layout: the desktop shell serves from tauri://
		// where a worker cannot register, and everything it would cache is already local.
		serviceWorker: {
			register: false
		},
		alias: {
			$lib: 'src/lib',
			$home: web ? 'src/lib/framer/Standalone.svelte' : 'src/routes/(enframe)/home-default.svelte',
			$editor: web ? 'src/routes/(enframe)/redirect-home.svelte' : 'src/routes/(enframe)/edit/library-editor.svelte',
			$library: web ? 'src/routes/(enframe)/redirect-home.svelte' : 'src/routes/(enframe)/library-browser.svelte',
			$carver: web ? 'src/routes/(enframe)/redirect-home.svelte' : 'src/routes/(enframe)/carve/library-carver.svelte'
		}
	}
};
