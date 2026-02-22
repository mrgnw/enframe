<script lang="ts">
import * as Command from '$lib/components/ui/command/index.js';

interface Props {
	onaddframe?: () => void;
	onaddpoint?: () => void;
	onsave?: () => void;
	onexport?: () => void;
	onsetoutputdir?: () => void;
}

let { onaddframe, onaddpoint, onsave, onexport, onsetoutputdir }: Props = $props();

let open = $state(false);

function handleKeydown(e: KeyboardEvent) {
	if (e.key === 'k' && (e.metaKey || e.ctrlKey)) {
		e.preventDefault();
		open = !open;
	}
}

function run(fn?: () => void) {
	open = false;
	fn?.();
}
</script>

<svelte:document onkeydown={handleKeydown} />

<Command.Dialog bind:open>
	<Command.Input placeholder="Type a command..." />
	<Command.List>
		<Command.Empty>No results found.</Command.Empty>
		<Command.Group heading="Actions">
			<Command.Item onSelect={() => run(onaddframe)}>
				<span>Add Frame</span>
			</Command.Item>
			<Command.Item onSelect={() => run(onaddpoint)}>
				<span>Add Focal Point</span>
			</Command.Item>
			<Command.Item onSelect={() => run(onsave)}>
				<span>Save</span>
				<Command.Shortcut>&#8984;S</Command.Shortcut>
			</Command.Item>
			<Command.Item onSelect={() => run(onexport)}>
				<span>Export</span>
			</Command.Item>
			<Command.Item onSelect={() => run(onsetoutputdir)}>
				<span>Set Output Directory</span>
			</Command.Item>
		</Command.Group>
	</Command.List>
</Command.Dialog>
