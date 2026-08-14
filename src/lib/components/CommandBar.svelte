<script lang="ts">
import { page } from '$app/stores';
import * as Command from '$lib/components/ui/command/index.js';
import * as Dialog from '$lib/components/ui/dialog/index.js';
import type { ExportFormat } from '$lib/framer/types';

interface Props {
	onaddframe?: () => void;
	onaddpoint?: () => void;
	ongenerate?: () => void;
	onsave?: () => void;
	onexport?: () => void;
	onsetoutputdir?: () => void;
	onnextframe?: () => void;
	onprevframe?: () => void;
	ondeleteframe?: () => void;
	onsetformat?: (format: ExportFormat) => void;
	onundo?: () => void;
	onredo?: () => void;
	ontoggle?: () => void;
	ondeselect?: () => void;
	onpreview?: () => void;
	onpreviewlayout?: () => void;
	oncycleguide?: () => void;
	onselectall?: () => void;
	onclearselection?: () => void;
	selectionCount?: number;
}

let {
	onaddframe,
	onaddpoint,
	ongenerate,
	onsave,
	onexport,
	onsetoutputdir,
	onnextframe,
	onprevframe,
	ondeleteframe,
	onsetformat,
	onundo,
	onredo,
	ontoggle,
	ondeselect,
	onpreview,
	onpreviewlayout,
	oncycleguide,
	onselectall,
	onclearselection,
	selectionCount = 0
}: Props = $props();

let open = $state(false);
let searchValue = $state('');

let showCheatsheet = $derived(searchValue.trim() === '');
let standalone = $derived($page.url.pathname.startsWith('/crop'));

function handleKeydown(e: KeyboardEvent) {
	if (e.key === 'k' && (e.metaKey || e.ctrlKey)) {
		e.preventDefault();
		open = !open;
		if (open) searchValue = '';
	} else if (e.key === '?' && !e.metaKey && !e.ctrlKey) {
		const el = document.activeElement;
		const tag = el?.tagName.toLowerCase();
		if (tag === 'input' || tag === 'textarea' || (el as HTMLElement)?.isContentEditable) return;
		e.preventDefault();
		open = true;
		searchValue = '';
	}
}

function run(fn?: () => void) {
	open = false;
	searchValue = '';
	fn?.();
}

const shortcuts = [
	{
		heading: 'General',
		items: [
			{ key: '\u2318K', label: 'Command bar' },
			{ key: '?', label: 'Keyboard shortcuts' },
			{ key: 'Esc', label: 'Deselect' },
			{ key: '\u2318Z', label: 'Undo' },
			{ key: '\u2318\u21e7Z', label: 'Redo' },
			{ key: '\u232b', label: 'Delete selected' }
		]
	},
	{
		heading: 'Selection',
		items: [
			{ key: 'Click', label: 'Select frame' },
			{ key: '\u21e7Click', label: 'Add / remove' },
			{ key: '\u2318A', label: 'Select all frames' },
			{ key: '\u21e7Click', label: 'Point range (list)' },
			{ key: '\u2318Click', label: 'Toggle point' },
			{ key: 'Esc', label: 'Select none' },
			{ key: 'Right-click', label: 'Frame actions' },
			{ key: '\u232b', label: 'Delete selection' }
		]
	},
	{
		heading: 'Actions',
		items: [
			{ key: 'F', label: 'Add frame' },
			{ key: 'P', label: 'Add focal point' },
			{ key: 'A', label: 'Generate versions' },
			{ key: '\u2325Click', label: 'Point + auto versions' },
			{ key: '\u2318S', label: 'Save' },
			{ key: '\u2318E', label: 'Export' },
			{ key: 'O', label: 'Set output dir' },
			{ key: 'T', label: 'Toggle panel' },
			{ key: 'V', label: 'Toggle preview' },
			{ key: '⇧V', label: 'Preview in main' },
			{ key: 'G', label: 'Cycle guide' }
		]
	},
	{
		heading: 'Navigation',
		items: [
			{ key: ',', label: 'Previous frame' },
			{ key: '.', label: 'Next frame' },
			{ key: '[', label: 'Previous point' },
			{ key: ']', label: 'Next point' },
			{ key: '1\u20139', label: 'Select frame' },
			{ key: '0', label: 'Fit / fill' },
			{ key: 'Scroll', label: 'Zoom' },
			{ key: 'Drag', label: 'Pan' }
		]
	}
];
</script>

<svelte:document onkeydown={handleKeydown} />

<Dialog.Root bind:open>
	<Dialog.Header class="sr-only">
		<Dialog.Title>Command Palette</Dialog.Title>
		<Dialog.Description>Search for a command to run</Dialog.Description>
	</Dialog.Header>
	<Dialog.Content class="overflow-hidden p-0">
		{#if showCheatsheet}
			<div class="flex flex-col">
				<div class="flex h-12 items-center gap-2 border-b ps-3 pe-8">
					<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="shrink-0 opacity-50"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>
					<input
						class="flex h-10 w-full bg-transparent py-3 text-sm outline-hidden placeholder:text-muted-foreground"
						placeholder="Type a command..."
						bind:value={searchValue}
					/>
				</div>
				<div class="grid grid-cols-3 gap-x-6 gap-y-1 p-3 px-4 pb-4">
					{#each shortcuts as group}
						<div class="flex flex-col gap-0.5">
							<h3 class="text-muted-foreground mb-1 text-[11px] font-semibold uppercase tracking-wide">{group.heading}</h3>
							{#each group.items as item}
								<div class="flex items-center gap-2 py-0.5">
									<kbd class="text-foreground bg-muted border-border inline-flex shrink-0 items-center justify-center rounded border px-1.5 text-[11px] font-medium min-w-6 h-[22px]">{item.key}</kbd>
									<span class="text-muted-foreground text-xs">{item.label}</span>
								</div>
							{/each}
						</div>
					{/each}
				</div>
			</div>
		{:else}
			<Command.Root
				class="**:data-[slot=command-input-wrapper]:h-12 [&_[data-command-group]]:px-2 [&_[data-command-group]:not([hidden])_~[data-command-group]]:pt-0 [&_[data-command-input-wrapper]_svg]:h-5 [&_[data-command-input-wrapper]_svg]:w-5 [&_[data-command-input]]:h-12 [&_[data-command-item]]:px-2 [&_[data-command-item]]:py-3 [&_[data-command-item]_svg]:h-5 [&_[data-command-item]_svg]:w-5"
			>
				<Command.Input placeholder="Type a command..." bind:value={searchValue} />
				<Command.List>
					<Command.Empty>No results found.</Command.Empty>
					<Command.Group heading="Actions">
						<Command.Item onSelect={() => run(onaddframe)}>
							<span>Add Frame</span>
							<Command.Shortcut>F</Command.Shortcut>
						</Command.Item>
						<Command.Item onSelect={() => run(onaddpoint)}>
							<span>Add Focal Point</span>
							<Command.Shortcut>P</Command.Shortcut>
						</Command.Item>
						<Command.Item onSelect={() => run(ongenerate)}>
							<span>Generate Versions</span>
							<Command.Shortcut>A</Command.Shortcut>
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
							<Command.Shortcut>O</Command.Shortcut>
						</Command.Item>
						<Command.Item onSelect={() => run(ontoggle)}>
							<span>Toggle Panel</span>
							<Command.Shortcut>T</Command.Shortcut>
						</Command.Item>
						<Command.Item onSelect={() => run(onpreview)}>
							<span>Toggle Preview</span>
							<Command.Shortcut>V</Command.Shortcut>
						</Command.Item>
						<Command.Item onSelect={() => run(onpreviewlayout)}>
							<span>Preview in Main Window</span>
							<Command.Shortcut>&#8679;V</Command.Shortcut>
						</Command.Item>
						<Command.Item onSelect={() => run(oncycleguide)}>
							<span>Cycle Composition Guide</span>
							<Command.Shortcut>G</Command.Shortcut>
						</Command.Item>
					</Command.Group>
					<Command.Group heading="Edit">
						<Command.Item onSelect={() => run(onundo)}>
							<span>Undo</span>
							<Command.Shortcut>&#8984;Z</Command.Shortcut>
						</Command.Item>
						<Command.Item onSelect={() => run(onredo)}>
							<span>Redo</span>
							<Command.Shortcut>&#8984;&#8679;Z</Command.Shortcut>
						</Command.Item>
						<Command.Item onSelect={() => run(ondeselect)}>
							<span>Deselect</span>
							<Command.Shortcut>Esc</Command.Shortcut>
						</Command.Item>
					</Command.Group>
					<Command.Group heading="Selection">
						<Command.Item onSelect={() => run(onselectall)}>
							<span>Select All Frames</span>
							<Command.Shortcut>&#8984;A</Command.Shortcut>
						</Command.Item>
						<Command.Item onSelect={() => run(onclearselection)}>
							<span>Select None</span>
							<Command.Shortcut>Esc</Command.Shortcut>
						</Command.Item>
						<Command.Item onSelect={() => run(ondeleteframe)}>
							<span>Delete Selected{selectionCount > 1 ? ` (${selectionCount})` : ''}</span>
							<Command.Shortcut>&#9003;</Command.Shortcut>
						</Command.Item>
					</Command.Group>
					<Command.Group heading="Navigation">
						<Command.Item onSelect={() => run(onnextframe)}>
							<span>Next Frame</span>
							<Command.Shortcut>.</Command.Shortcut>
						</Command.Item>
						<Command.Item onSelect={() => run(onprevframe)}>
							<span>Previous Frame</span>
							<Command.Shortcut>,</Command.Shortcut>
						</Command.Item>
					</Command.Group>
					{#if !standalone}
						<Command.Group heading="Open">
							<Command.LinkItem href="/" onSelect={() => (open = false)}>
								<span>Open Single Image&hellip;</span>
							</Command.LinkItem>
						</Command.Group>
					{/if}
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
			</Command.Root>
		{/if}
	</Dialog.Content>
</Dialog.Root>


