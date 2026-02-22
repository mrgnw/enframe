<script lang="ts">
import type { Frame } from '$lib/db';
import { updateFrame } from '$lib/db';

interface Props {
	frame: Frame;
	imageWidth: number;
	imageHeight: number;
	scale: number;
}

let { frame, imageWidth, imageHeight, scale }: Props = $props();

let isDragging = $state(false);
let isResizing = $state(false);
let resizeHandle = $state<string | null>(null);
let dragStart = $state({ x: 0, y: 0, frameX: 0, frameY: 0, frameW: 0, frameH: 0 });

function handleFramePointerDown(event: PointerEvent) {
	if (event.button !== 0) return;
	event.stopPropagation();

	isDragging = true;
	dragStart = {
		x: event.clientX,
		y: event.clientY,
		frameX: frame.x,
		frameY: frame.y,
		frameW: frame.width,
		frameH: frame.height
	};
	(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function handleFramePointerMove(event: PointerEvent) {
	if (!isDragging || isResizing) return;
	event.stopPropagation();

	const deltaX = (event.clientX - dragStart.x) / scale / imageWidth;
	const deltaY = (event.clientY - dragStart.y) / scale / imageHeight;

	const newX = Math.max(0, Math.min(1 - frame.width, dragStart.frameX + deltaX));
	const newY = Math.max(0, Math.min(1 - frame.height, dragStart.frameY + deltaY));

	frame.x = newX;
	frame.y = newY;
}

async function handleFramePointerUp(event: PointerEvent) {
	if (!isDragging) return;
	event.stopPropagation();

	isDragging = false;
	(event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);

	await updateFrame(frame.id, { x: frame.x, y: frame.y });
}

function handleHandlePointerDown(event: PointerEvent, handle: string) {
	if (event.button !== 0) return;
	event.stopPropagation();

	isResizing = true;
	resizeHandle = handle;
	dragStart = {
		x: event.clientX,
		y: event.clientY,
		frameX: frame.x,
		frameY: frame.y,
		frameW: frame.width,
		frameH: frame.height
	};
	(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function handleHandlePointerMove(event: PointerEvent) {
	if (!isResizing || !resizeHandle) return;
	event.stopPropagation();

	const deltaX = (event.clientX - dragStart.x) / scale / imageWidth;
	const deltaY = (event.clientY - dragStart.y) / scale / imageHeight;

	let newX = dragStart.frameX;
	let newY = dragStart.frameY;
	let newW = dragStart.frameW;
	let newH = dragStart.frameH;

	if (resizeHandle.includes('w')) {
		const change = Math.min(deltaX, dragStart.frameW - 0.05);
		newX = dragStart.frameX + change;
		newW = dragStart.frameW - change;
	} else if (resizeHandle.includes('e')) {
		newW = Math.max(0.05, Math.min(1 - newX, dragStart.frameW + deltaX));
	}

	if (resizeHandle.includes('n')) {
		const change = Math.min(deltaY, dragStart.frameH - 0.05);
		newY = dragStart.frameY + change;
		newH = dragStart.frameH - change;
	} else if (resizeHandle.includes('s')) {
		newH = Math.max(0.05, Math.min(1 - newY, dragStart.frameH + deltaY));
	}

	frame.x = Math.max(0, newX);
	frame.y = Math.max(0, newY);
	frame.width = newW;
	frame.height = newH;
}

async function handleHandlePointerUp(event: PointerEvent) {
	if (!isResizing) return;
	event.stopPropagation();

	isResizing = false;
	resizeHandle = null;
	(event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);

	await updateFrame(frame.id, { x: frame.x, y: frame.y, width: frame.width, height: frame.height });
}
</script>

<div
	class="frame-overlay"
	class:dragging={isDragging}
	class:resizing={isResizing}
	style="
		left: {frame.x * 100}%;
		top: {frame.y * 100}%;
		width: {frame.width * 100}%;
		height: {frame.height * 100}%;
	"
	role="button"
	tabindex="0"
	onpointerdown={handleFramePointerDown}
	onpointermove={handleFramePointerMove}
	onpointerup={handleFramePointerUp}
>
	{#if frame.label}
		<div class="frame-label">{frame.label}</div>
	{/if}

	<!-- Resize handles -->
	<div class="handle nw" role="button" tabindex="-1" aria-label="Resize northwest" onpointerdown={(e) => handleHandlePointerDown(e, 'nw')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
	<div class="handle ne" role="button" tabindex="-1" aria-label="Resize northeast" onpointerdown={(e) => handleHandlePointerDown(e, 'ne')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
	<div class="handle sw" role="button" tabindex="-1" aria-label="Resize southwest" onpointerdown={(e) => handleHandlePointerDown(e, 'sw')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
	<div class="handle se" role="button" tabindex="-1" aria-label="Resize southeast" onpointerdown={(e) => handleHandlePointerDown(e, 'se')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
	<div class="handle n" role="button" tabindex="-1" aria-label="Resize north" onpointerdown={(e) => handleHandlePointerDown(e, 'n')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
	<div class="handle s" role="button" tabindex="-1" aria-label="Resize south" onpointerdown={(e) => handleHandlePointerDown(e, 's')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
	<div class="handle w" role="button" tabindex="-1" aria-label="Resize west" onpointerdown={(e) => handleHandlePointerDown(e, 'w')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
	<div class="handle e" role="button" tabindex="-1" aria-label="Resize east" onpointerdown={(e) => handleHandlePointerDown(e, 'e')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
</div>

<style>
	.frame-overlay {
		position: absolute;
		border: 2px solid #3b82f6;
		box-sizing: border-box;
		cursor: move;
		pointer-events: auto;
	}

	.frame-overlay:hover {
		border-color: #2563eb;
		border-width: 3px;
		z-index: 5;
	}

	.frame-overlay.dragging,
	.frame-overlay.resizing {
		border-color: #1d4ed8;
		border-width: 3px;
		z-index: 10;
	}

	.frame-label {
		position: absolute;
		top: -1.75rem;
		left: 0;
		background: #3b82f6;
		color: white;
		padding: 0.25rem 0.5rem;
		font-size: 0.75rem;
		border-radius: 0.25rem;
		white-space: nowrap;
		pointer-events: none;
	}

	.handle {
		position: absolute;
		width: 10px;
		height: 10px;
		background: white;
		border: 2px solid #3b82f6;
		box-sizing: border-box;
	}

	.handle:hover {
		background: #3b82f6;
	}

	.handle.nw { top: -5px; left: -5px; cursor: nw-resize; }
	.handle.ne { top: -5px; right: -5px; cursor: ne-resize; }
	.handle.sw { bottom: -5px; left: -5px; cursor: sw-resize; }
	.handle.se { bottom: -5px; right: -5px; cursor: se-resize; }
	.handle.n { top: -5px; left: 50%; transform: translateX(-50%); cursor: n-resize; }
	.handle.s { bottom: -5px; left: 50%; transform: translateX(-50%); cursor: s-resize; }
	.handle.w { top: 50%; left: -5px; transform: translateY(-50%); cursor: w-resize; }
	.handle.e { top: 50%; right: -5px; transform: translateY(-50%); cursor: e-resize; }
</style>
