<script lang="ts">
import { Check, ChevronDown, ChevronUp, Circle, Crop, Redo2, Undo2 } from '@lucide/svelte';

interface Props {
	canUndo?: boolean;
	canRedo?: boolean;
	saveStatus?: 'saved' | 'saving' | 'unsaved';
	panelExpanded?: boolean;
	onundo?: () => void;
	onredo?: () => void;
	onaddframe?: () => void;
	onaddpoint?: () => void;
	onsave?: () => void;
	ontoggle?: () => void;
}

let {
	canUndo = false,
	canRedo = false,
	saveStatus = 'saved',
	panelExpanded = false,
	onundo,
	onredo,
	onaddframe,
	onaddpoint,
	onsave,
	ontoggle
}: Props = $props();
</script>

<div class="floating-toolbar">
	<div class="toolbar-group">
		<button class="tb-btn" disabled={!canUndo} onclick={onundo} title="Undo (⌘Z)">
			<Undo2 size={16} />
		</button>
		<button class="tb-btn" disabled={!canRedo} onclick={onredo} title="Redo (⌘⇧Z)">
			<Redo2 size={16} />
		</button>
	</div>

	<div class="toolbar-divider"></div>

	<div class="toolbar-group">
		<button class="tb-btn" onclick={onaddframe} title="Add Frame (F)">
			<span class="btn-icon-combo">
				<span class="plus">+</span>
				<Crop size={14} />
			</span>
		</button>
		<button class="tb-btn" onclick={onaddpoint} title="Add Focal Point (P)">
			<span class="btn-icon-combo">
				<span class="plus">+</span>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
					<path d="M5.616 20q-.691 0-1.153-.462T4 18.384V15h1v3.385q0 .23.192.423t.423.192H9v1zM15 20v-1h3.385q.23 0 .423-.192t.192-.424V15h1v3.385q0 .69-.462 1.152T18.384 20zM4 9V5.616q0-.691.463-1.153T5.616 4H9v1H5.616q-.231 0-.424.192T5 5.616V9zm15 0V5.616q0-.231-.192-.424T18.384 5H15V4h3.385q.69 0 1.152.463T20 5.616V9zm-9.835 5.836Q8 13.67 8 12t1.165-2.835T12 8t2.836 1.165T16 12t-1.164 2.836T12 16t-2.835-1.164" />
				</svg>
			</span>
		</button>
	</div>

	<div class="toolbar-divider"></div>

	<button
		class="tb-btn"
		class:save-saved={saveStatus === 'saved'}
		class:save-unsaved={saveStatus === 'unsaved'}
		class:save-saving={saveStatus === 'saving'}
		disabled={saveStatus === 'saving'}
		onclick={onsave}
		title="Save (⌘S)"
	>
		{#if saveStatus === 'saved'}
			<Check size={16} />
		{:else if saveStatus === 'saving'}
			<Circle size={16} class="spin" />
		{:else}
			<Circle size={8} />
		{/if}
	</button>

	<div class="toolbar-divider"></div>

	<button class="tb-btn toggle-btn" onclick={ontoggle} title={panelExpanded ? 'Collapse Panel (T)' : 'Expand Panel (T)'}>
		{#if panelExpanded}
			<ChevronUp size={16} />
		{:else}
			<ChevronDown size={16} />
		{/if}
	</button>
</div>

<style>
	.floating-toolbar {
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 4px;
		background: rgba(0, 0, 0, 0.7);
		backdrop-filter: blur(12px);
		border-radius: 999px;
		border: 1px solid rgba(255, 255, 255, 0.1);
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
	}

	.toolbar-group {
		display: flex;
		align-items: center;
		gap: 2px;
	}

	.toolbar-divider {
		width: 1px;
		height: 20px;
		background: rgba(255, 255, 255, 0.15);
		margin: 0 2px;
	}

	.tb-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		border: none;
		border-radius: 999px;
		background: transparent;
		color: rgba(255, 255, 255, 0.8);
		cursor: pointer;
		transition: background 0.15s, color 0.15s;
	}

	.tb-btn:hover:not(:disabled) {
		background: rgba(255, 255, 255, 0.15);
		color: white;
	}

	.tb-btn:disabled {
		opacity: 0.3;
		cursor: default;
	}

	.btn-icon-combo {
		display: flex;
		align-items: center;
		gap: 1px;
	}

	.plus {
		font-size: 12px;
		font-weight: 600;
		line-height: 1;
	}

	.save-saved {
		color: rgba(74, 222, 128, 0.8);
	}

	.save-saved:hover {
		color: #4ade80;
	}

	.save-unsaved {
		color: #f59e0b;
	}

	.save-unsaved:hover {
		color: #fbbf24;
	}

	.save-saving {
		color: rgba(255, 255, 255, 0.4);
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}
</style>
