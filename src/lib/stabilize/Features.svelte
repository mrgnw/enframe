<script lang="ts">
// The features panel: the folder's features in their colours, the one on screen picked ({ and } cycle), and
// new, rename and delete. Each feature is its own clip.
import type { Sequence } from './sequence.svelte';

let { sequence }: { sequence: Sequence } = $props();

let renaming = $state(false);
let draft = $state('');
const refusal = $derived(renaming ? sequence.refusal(draft) : null);

function startRename() {
	draft = sequence.session.name;
	renaming = true;
}

function finishRename() {
	if (renaming && !refusal) sequence.rename(draft);
	renaming = false;
}

function key(e: KeyboardEvent) {
	if (e.key === 'Enter') finishRename();
	else if (e.key === 'Escape') renaming = false;
}
</script>

<div class="island panel features desk" id="features">
	<ul>
		{#each sequence.sessions as s, i (s)}
			<li class:on={i === sequence.active} style:--hue="{s.hue}deg">
				{#if renaming && i === sequence.active}
					<i class="swatch"></i>
					<!-- svelte-ignore a11y_autofocus -->
					<input class="name" bind:value={draft} onkeydown={key} onblur={finishRename} spellcheck="false" aria-label="Feature name" autofocus />
				{:else}
					<button class="pick" title="Show this feature ({'{'} and {'}'} cycle)" onclick={() => sequence.select(i)} ondblclick={startRename}>
						<i class="swatch"></i>{s.name}
					</button>
				{/if}
			</li>
		{/each}
	</ul>
	{#if refusal}
		<div class="note warn" role="alert">{refusal}</div>
	{/if}
	<div class="actions">
		<button class="btn" id="new-feature" title="Mark another thing in the same photos: a clip of its own" onclick={() => sequence.add()}>New</button>
		<button class="btn" id="rename-feature" onclick={startRename}>Rename</button>
		<button class="btn" id="delete-feature" title={sequence.canRemove ? 'Delete this feature and its marks' : 'Another feature must have points first'} disabled={!sequence.canRemove} onclick={() => confirm(`Delete ${sequence.session.name} and its marks?`) && sequence.remove()}>Delete</button>
	</div>
</div>
