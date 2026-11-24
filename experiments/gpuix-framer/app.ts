import { render_hot } from 'gpuix-svelte';

const photo = new URL('./photo.jpg', import.meta.url).pathname;

render_hot(new URL('./App.svelte', import.meta.url), {
	title: 'gpuix framer spike',
	width: 960,
	height: 600,
	props: { photo, photoWidth: 1200, photoHeight: 800 }
});
