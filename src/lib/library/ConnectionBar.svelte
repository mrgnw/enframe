<script lang="ts">
import { connectTo, currentConnection, knownConnections } from '$lib/bridge';

let url = $state('');
let busy = $state(false);
let error = $state<string | null>(null);

const others = knownConnections().filter((known) => known !== currentConnection());

// Everything on screen belongs to the instance we are leaving, so reload rather than
// invalidate a dozen lists by hand.
async function connect(target: string) {
	busy = true;
	error = null;
	try {
		await connectTo(target);
		location.reload();
	} catch (failure) {
		error = failure instanceof Error ? failure.message : 'Could not connect';
		busy = false;
	}
}
</script>

<form
	class="connection"
	onsubmit={(event) => {
		event.preventDefault();
		void connect(url);
	}}
>
	<div class="row">
		<input
			type="text"
			placeholder="192.168.1.9 or 100.64.0.7:5911"
			bind:value={url}
			disabled={busy}
			spellcheck="false"
			autocapitalize="off"
			autocorrect="off"
		/>
		<button type="submit" disabled={busy || url.trim().length === 0}>
			{busy ? 'Connecting…' : 'Connect'}
		</button>
	</div>

	{#if error}
		<p class="error">{error}</p>
	{/if}

	{#if others.length > 0}
		<ul class="known">
			{#each others as instance (instance)}
				<li>
					<button type="button" disabled={busy} onclick={() => connect(instance)}>{instance}</button>
				</li>
			{/each}
		</ul>
	{/if}
</form>

<style>
	.connection {
		max-width: 22rem;
		margin: 1.25rem auto 0;
		text-align: left;
	}

	.row {
		display: flex;
		gap: 0.5rem;
	}

	input {
		flex: 1;
		min-width: 0;
		padding: 0.4rem 0.6rem;
		background: #1e1e1e;
		border: 1px solid #2e2e2e;
		border-radius: 7px;
		color: #e4e4e7;
		font-size: 0.8rem;
	}

	input:focus {
		outline: none;
		border-color: #2563eb;
	}

	button[type='submit'] {
		padding: 0.4rem 0.8rem;
		background: #2563eb;
		border: none;
		border-radius: 7px;
		color: white;
		font-size: 0.8rem;
		cursor: pointer;
	}

	button:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.error {
		margin: 0.5rem 0 0;
		color: #ef4444;
		font-size: 0.75rem;
	}

	.known {
		margin: 0.75rem 0 0;
		padding: 0;
		list-style: none;
		display: flex;
		flex-wrap: wrap;
		gap: 0.375rem;
	}

	.known button {
		padding: 0.2rem 0.5rem;
		background: none;
		border: 1px solid #2e2e2e;
		border-radius: 6px;
		color: #a1a1aa;
		font-size: 0.72rem;
		cursor: pointer;
	}

	.known button:hover:not(:disabled) {
		color: #e4e4e7;
		border-color: #555;
	}
</style>
