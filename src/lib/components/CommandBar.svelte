<script lang="ts">
import * as Command from '$lib/components/ui/command/index.js';
import type { ExportFormat } from '$lib/export';

interface Props {
	onaddframe?: () => void;
	onaddpoint?: () => void;
	onsave?: () => void;
	onexport?: () => void;
	onsetoutputdir?: () => void;
	onnextframe?: () => void;
	onprevframe?: () => void;
	ondeleteframe?: () => void;
	onsetformat?: (format: ExportFormat) => void;
}

let {
	onaddframe,
	onaddpoint,
	onsave,
	onexport,
	onsetoutputdir,
	onnextframe,
	onprevframe,
	ondeleteframe,
	onsetformat
}: Props = $props();

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
				<Command.Shortcut>&#8984;E</Command.Shortcut>
			</Command.Item>
			<Command.Item onSelect={() => run(onsetoutputdir)}>
				<span>Set Output Directory</span>
			</Command.Item>
		</Command.Group>
		<Command.Group heading="Frames">
			<Command.Item onSelect={() => run(onnextframe)}>
				<span>Next Frame</span>
				<Command.Shortcut>.</Command.Shortcut>
			</Command.Item>
			<Command.Item onSelect={() => run(onprevframe)}>
				<span>Previous Frame</span>
				<Command.Shortcut>,</Command.Shortcut>
			</Command.Item>
			<Command.Item onSelect={() => run(ondeleteframe)}>
				<span>Delete Frame</span>
				<Command.Shortcut>&#9003;</Command.Shortcut>
			</Command.Item>
		</Command.Group>
		<Command.Group heading="Export Format">
			<Command.Item onSelect={() => { open = false; onsetformat?.('avif'); }}>
				<span>AVIF</span>
			</Command.Item>
			<Command.Item onSelect={() => { open = false; onsetformat?.('jpg'); }}>
				<span>JPG</span>
			</Command.Item>
			<Command.Item onSelect={() => { open = false; onsetformat?.('png'); }}>
				<span>PNG</span>
			</Command.Item>
		</Command.Group>
	</Command.List>
</Command.Dialog>
