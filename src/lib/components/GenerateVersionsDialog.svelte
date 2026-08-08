<script lang="ts">
import { DESTINATIONS } from '$lib/destinations';

interface Props {
	open: boolean;
	onclose: () => void;
	ongenerate: (slugs: string[]) => void;
}

let { open, onclose, ongenerate }: Props = $props();

let selected = $state<Set<string>>(new Set(DESTINATIONS.map((d) => d.slug)));

function toggle(slug: string) {
	const next = new Set(selected);
	if (next.has(slug)) {
		next.delete(slug);
	} else {
		next.add(slug);
	}
	selected = next;
}

function submit() {
	ongenerate(DESTINATIONS.filter((d) => selected.has(d.slug)).map((d) => d.slug));
	onclose();
}
</script>

{#if open}
	<dialog
		class="gen-dialog"
		onclose={onclose}
		{@attach (node: HTMLDialogElement) => {
			node.showModal();
		}}
	>
		<h2>Generate versions</h2>
		<p class="hint">One auto-fitted frame per destination. Required focal points stay in shot.</p>

		<ul class="dest-list">
			{#each DESTINATIONS as destination (destination.slug)}
				<li>
					<label class="dest-item">
						<input
							type="checkbox"
							checked={selected.has(destination.slug)}
							onchange={() => toggle(destination.slug)}
						/>
						<span class="name">{destination.name}</span>
						<span class="ratio">{destination.ratio.w}:{destination.ratio.h}</span>
					</label>
				</li>
			{/each}
		</ul>

		<div class="actions">
			<button class="ghost" onclick={onclose}>Cancel</button>
			<button class="primary" disabled={selected.size === 0} onclick={submit}>
				Generate {selected.size}
			</button>
		</div>
	</dialog>
{/if}

<style>
	.gen-dialog {
		width: 320px;
		padding: 16px;
		border: 1px solid rgba(255, 255, 255, 0.12);
		border-radius: 14px;
		background: rgba(20, 20, 20, 0.95);
		backdrop-filter: blur(16px);
		color: white;
		box-shadow: 0 16px 48px rgba(0, 0, 0, 0.55);
	}

	.gen-dialog::backdrop {
		background: rgba(0, 0, 0, 0.5);
	}

	h2 {
		margin: 0;
		font-size: 14px;
		font-weight: 600;
	}

	.hint {
		margin: 4px 0 12px;
		font-size: 11px;
		line-height: 1.4;
		color: rgba(255, 255, 255, 0.45);
	}

	.dest-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.dest-item {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 8px;
		border-radius: 8px;
		font-size: 12px;
		cursor: pointer;
	}

	.dest-item:hover {
		background: rgba(255, 255, 255, 0.06);
	}

	.dest-item input {
		accent-color: #f59e0b;
		cursor: pointer;
	}

	.name {
		font-weight: 500;
	}

	.ratio {
		margin-left: auto;
		font-size: 10px;
		color: rgba(255, 255, 255, 0.4);
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: 6px;
		margin-top: 14px;
	}

	.actions button {
		padding: 6px 12px;
		border-radius: 8px;
		border: 1px solid transparent;
		font-family: inherit;
		font-size: 12px;
		font-weight: 600;
		cursor: pointer;
	}

	.ghost {
		background: transparent;
		border-color: rgba(255, 255, 255, 0.15);
		color: rgba(255, 255, 255, 0.7);
	}

	.ghost:hover {
		color: white;
		border-color: rgba(255, 255, 255, 0.3);
	}

	.primary {
		background: #f59e0b;
		color: #1a1a1a;
	}

	.primary:hover:not(:disabled) {
		background: #fbbf24;
	}

	.primary:disabled {
		opacity: 0.4;
		cursor: default;
	}
</style>
