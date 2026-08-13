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
		alias: {
			$lib: 'src/lib',
			$home: web ? 'src/routes/crop/+page.svelte' : 'src/routes/home-library.svelte',
			$editor: web ? 'src/routes/redirect-home.svelte' : 'src/routes/edit/library-editor.svelte'
		}
	}
};
