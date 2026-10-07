<script lang="ts">
	// M1's placeholder: the endpoints and the document they take. M2 replaces this with the marking screen.
	const example = {
		folder: 'samples/train clouds',
		marks: {
			points: [
				[675, 345],
				[985, 345]
			],
			at: { 'IMG_4703.avif': { points: [[846, 641.5]], zoom: 2.5 } },
			skip: ['IMG_4721.avif']
		}
	};
</script>

<main>
	<h1>pinframe</h1>
	<p>Pin one feature in place across a photo sequence. This server wraps the pinframe CLI.</p>

	<h2>Endpoints</h2>
	<dl>
		<dt><code>POST /api/track</code></dt>
		<dd>
			Body <code>{'{folder, marks}'}</code>. Saves the marks to <code>&lt;folder&gt;.pin.json</code>,
			tracks (JSON-only, with the feature cache) and returns the sidecar. A newer request for the same
			folder kills the one in flight, which answers <code>409</code>.
		</dd>
		<dt><code>POST /api/export</code></dt>
		<dd>
			Body <code>{'{folder, marks, out?, pad?, fps?, frames?, vf?}'}</code>. Renders the clip
			(default <code>&lt;folder&gt;.avif</code>, the extension picks the format) and streams
			server-sent events: <code>progress</code> lines, then <code>done</code> with
			<code>{'{out, sidecar}'}</code>, or <code>error</code>.
		</dd>
	</dl>

	<h2>Document</h2>
	<p>
		<code>marks</code> mirrors the CLI: <code>points</code> on the first photo, <code>at</code> per
		photo with optional <code>zoom</code>, <code>skip</code>, <code>order</code>. Coordinates are
		pixels of the displayed (oriented) photo. <code>folder</code> is absolute or relative to the
		repository.
	</p>
	<pre>{JSON.stringify(example, null, 2)}</pre>
</main>

<style>
	main {
		max-width: 48rem;
		margin: 2rem auto;
		padding: 0 1rem;
		font-family: system-ui, sans-serif;
		line-height: 1.5;
	}
	code,
	pre {
		font-family: ui-monospace, monospace;
		font-size: 0.9em;
	}
	pre {
		padding: 0.75rem;
		background: #f4f4f4;
		overflow-x: auto;
	}
	dt {
		margin-top: 0.75rem;
	}
</style>
