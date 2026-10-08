<script lang="ts">
import { goto } from '$app/navigation';
import { slide } from 'svelte/transition';

type AppId = 'enframe' | 'stabilize';

let { current, responsive = false }: { current: AppId; responsive?: boolean } = $props();

const APPS: { id: AppId; label: string; href: string; key: string }[] = [
	{ id: 'enframe', label: 'enframe', href: '/', key: '⌘1' },
	{ id: 'stabilize', label: 'Stabilize', href: '/stabilize', key: '⌘2' }
];

let open = $state(false);
let root: HTMLDivElement | undefined = $state();

function close() {
	open = false;
}

function inField(target: EventTarget | null) {
	const el = target as HTMLElement | null;
	const tag = el?.tagName.toLowerCase();
	return tag === 'input' || tag === 'textarea' || el?.isContentEditable;
}

function onDocumentClick(e: MouseEvent) {
	if (open && root && !root.contains(e.target as Node)) close();
}

function onDocumentKeydown(e: KeyboardEvent) {
	if (e.key === 'Escape' && open) {
		close();
		return;
	}
	if ((e.metaKey || e.ctrlKey) && (e.key === '1' || e.key === '2')) {
		if (inField(document.activeElement)) return;
		e.preventDefault();
		close();
		void goto(e.key === '1' ? '/' : '/stabilize');
	}
}
</script>

<svelte:document onclick={onDocumentClick} onkeydown={onDocumentKeydown} />

<div class="app-switch" class:dark={current === 'stabilize'} class:responsive bind:this={root}>
	<button
		type="button"
		class="trigger full"
		class:open
		aria-haspopup="menu"
		aria-expanded={open}
		onclick={() => (open = !open)}
	>
		<span class="word">{current === 'enframe' ? 'enframe' : 'Stabilize'}</span>
		<svg class="caret" viewBox="0 0 8 8" aria-hidden="true">
			<path d="M1 2.5l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.4" />
		</svg>
	</button>
	{#if responsive}
		<button
			type="button"
			class="trigger compact"
			class:open
			aria-haspopup="menu"
			aria-expanded={open}
			aria-label={`Switch app (currently ${current === 'enframe' ? 'enframe' : 'Stabilize'})`}
			onclick={() => (open = !open)}
		>
			<span class="initial">{current === 'enframe' ? 'e' : 'S'}</span>
		</button>
	{/if}
	{#if open}
		<div class="menu" role="menu" transition:slide={{ duration: 150 }}>
			{#each APPS as app (app.id)}
				{#if app.id === current}
					<span class="item current" role="menuitem" aria-current="true">{app.label}</span>
				{:else}
					<a class="item" role="menuitem" href={app.href} onclick={close}>{app.label}<kbd>{app.key}</kbd></a>
				{/if}
			{/each}
		</div>
	{/if}
</div>

<style>
	.app-switch {
		position: relative;
		display: inline-flex;
	}

	.trigger {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		cursor: pointer;
		user-select: none;
		background: none;
		border: 0;
		padding: 0;
		color: inherit;
		font: 600 15px/1 inherit;
		letter-spacing: -0.02em;
	}

	.app-switch.dark .trigger {
		font: 600 16px/1 var(--display);
	}

	.trigger.compact {
		display: none;
		width: 28px;
		height: 28px;
		border-radius: 50%;
		background: var(--surface);
		border: 1px solid var(--border);
		justify-content: center;
	}

	@media (max-width: 700px) {
		.app-switch.responsive .trigger.full {
			display: none;
		}
		.app-switch.responsive .trigger.compact {
			display: inline-flex;
		}
	}

	.initial {
		font: 600 13px/1 var(--display);
	}

	.caret {
		width: 8px;
		height: 8px;
		opacity: 0.45;
		transition: opacity 0.15s, rotate 0.2s;
	}

	.trigger:hover .caret,
	.trigger.open .caret {
		opacity: 0.9;
	}

	.trigger.open .caret {
		rotate: 180deg;
	}

	.menu {
		position: absolute;
		top: calc(100% + 10px);
		left: -8px;
		min-width: 164px;
		padding: 5px;
		border-radius: 12px;
		border: 1px solid var(--border);
		background: var(--card);
		backdrop-filter: blur(14px);
		box-shadow: 0 2px 12px rgba(20, 24, 40, 0.08);
		z-index: 50;
	}

	.app-switch.dark .menu {
		background: var(--surface);
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.45);
	}

	.item {
		display: flex;
		align-items: center;
		gap: 9px;
		padding: 7px 9px;
		border-radius: 8px;
		font: 500 13.5px/1 inherit;
		color: var(--foreground);
		text-decoration: none;
	}

	.app-switch.dark .item {
		color: var(--fg);
	}

	a.item:hover {
		background: color-mix(in oklch, var(--foreground) 8%, transparent);
	}

	.app-switch.dark a.item:hover {
		background: color-mix(in oklch, var(--fg) 8%, transparent);
	}

	.item.current {
		color: var(--muted-foreground);
		cursor: default;
	}

	.app-switch.dark .item.current {
		color: var(--muted);
	}

	.item.current::after {
		content: '\2713';
		margin-left: auto;
		font-size: 11px;
	}

	.item kbd {
		margin-left: auto;
		font: 500 10.5px/1 inherit;
		color: var(--muted-foreground);
	}

	.app-switch.dark .item kbd {
		color: var(--muted);
	}
</style>
