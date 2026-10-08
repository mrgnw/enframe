<script lang="ts">
	import favicon from '#lib/assets/favicon.svg';
	import '#lib/marking/pin.css';
	import '#lib/marking/marking.css';
	import '#lib/preview/preview.css';

	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{@render children()}
