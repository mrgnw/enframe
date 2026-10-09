<script lang="ts">
import { goto } from '$app/navigation';
import { slide } from 'svelte/transition';

type AppId = 'enframe' | 'stabilize';

let { current, responsive = false }: { current: AppId; responsive?: boolean } = $props();

const APPS: { id: AppId; label: string; href: string }[] = [
	{ id: 'enframe', label: 'enframe', href: '/' },
	{ id: 'stabilize', label: 'Stabilize', href: '/stabilize' }
];

// The browser keeps ⌘1–⌘9 for its tabs. Neither app uses the backquote, and with two apps one key is enough:
// it goes to the other one.
const KEY = '`';
const other = $derived(current === 'enframe' ? '/stabilize' : '/');

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
	if (e.key === KEY && !e.metaKey && !e.ctrlKey && !e.altKey) {
		if (inField(document.activeElement)) return;
		e.preventDefault();
		close();
		void goto(other);
	}
}
</script>

<svelte:document onclick={onDocumentClick} onkeydown={onDocumentKeydown} />

<div class="app-switch" class:responsive bind:this={root}>
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
					<a class="item" role="menuitem" href={app.href} onclick={close}>{app.label}<kbd>{KEY}</kbd></a>
				{/if}
			{/each}
		</div>
	{/if}
</div>

<style>
	/* The same in both apps: it sits in the first island of each layout's dock, inside `.app`'s tokens. */
	.app-switch {
		position: relative;
		display: inline-flex;
		/* Crossing between the apps, the switch holds still while the screens cross-fade around it. */
		view-transition-name: app-switch;
	}

	.trigger {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		cursor: pointer;
		user-select: none;
		background: none;
		border: 0;
		padding: 0 4px 0 0;
		color: var(--fg);
		font: inherit;
		font-size: 15px;
		font-weight: 600;
		line-height: 1;
		letter-spacing: -0.02em;
	}

	.trigger.compact {
		display: none;
		width: 28px;
		height: 28px;
		padding: 0;
		border-radius: 50%;
		background: var(--raised);
		justify-content: center;
		font-size: 13px;
	}

	@media (max-width: 700px) {
		.app-switch.responsive .trigger.full {
			display: none;
		}
		.app-switch.responsive .trigger.compact {
			display: inline-flex;
		}
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
		top: calc(100% + 16px);
		left: -13px;
		min-width: 164px;
		padding: 5px;
		border-radius: 12px;
		border: 1px solid var(--edge);
		background: var(--surface);
		backdrop-filter: blur(14px);
		box-shadow: var(--shadow-lg);
		z-index: 50;
	}

	.item {
		display: flex;
		align-items: center;
		gap: 9px;
		padding: 7px 9px;
		border-radius: 7px;
		color: var(--fg);
		font-size: 13px;
		font-weight: 500;
		line-height: 1;
		text-decoration: none;
	}

	a.item:hover {
		background: var(--raised);
	}

	.item.current {
		color: var(--subtle);
		cursor: default;
	}

	.item.current::after {
		content: '\2713';
		margin-left: auto;
		font-size: 11px;
	}

	.item kbd {
		margin-left: auto;
		padding: 0;
		border: 0;
		background: none;
		color: var(--subtle);
		font: inherit;
		font-size: 11px;
	}
</style>
