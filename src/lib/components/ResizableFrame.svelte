<script lang="ts">
import type { Frame } from '$lib/framer/types';

interface Props {
	frame: Frame;
	index?: number;
	imageWidth: number;
	imageHeight: number;
	scale: number;
	selected?: boolean;
	exportChecked?: boolean;
	aspectLocked?: boolean;
	compositionLabel?: string;
	clipsRequired?: boolean;
	onclaim?: (geometry: { x: number; y: number; width?: number; height?: number }) => Promise<void> | void;
	onchange?: () => void;
}

let {
	frame,
	index = 0,
	imageWidth,
	imageHeight,
	scale,
	selected = false,
	exportChecked = true,
	aspectLocked = false,
	compositionLabel = '',
	clipsRequired = false,
	onclaim,
	onchange
}: Props = $props();

let isAuto = $derived(frame.is_auto);

let handleSize = $derived(Math.max(6, 10 / scale));
let borderWidth = $derived(Math.max(1, 2 / scale));
let handleOffset = $derived(-handleSize / 2);
let labelScale = $derived(1 / scale);
let hs = $derived(`width:${handleSize}px;height:${handleSize}px;border-width:${borderWidth}px;`);

const DRAG_THRESHOLD = 3;

let isDragging = $state(false);
let isResizing = $state(false);
let resizeHandle = $state<string | null>(null);
let dragMoved = false;
let dragStart = $state({ x: 0, y: 0, frameX: 0, frameY: 0, frameW: 0, frameH: 0 });

function handleFramePointerDown(event: PointerEvent) {
	if (event.button !== 0) return;

	isDragging = true;
	dragMoved = false;
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

	if (!dragMoved) {
		const movedX = Math.abs(event.clientX - dragStart.x);
		const movedY = Math.abs(event.clientY - dragStart.y);
		if (movedX < DRAG_THRESHOLD && movedY < DRAG_THRESHOLD) return;
		dragMoved = true;
	}

	const deltaX = (event.clientX - dragStart.x) / scale / imageWidth;
	const deltaY = (event.clientY - dragStart.y) / scale / imageHeight;

	const newX = Math.max(0, Math.min(1 - frame.width, dragStart.frameX + deltaX));
	const newY = Math.max(0, Math.min(1 - frame.height, dragStart.frameY + deltaY));

	frame.x = newX;
	frame.y = newY;
}

// A press that never moved is not a drag: let it bubble to the canvas, which cycles the
// selection to the next frame under the cursor.
async function handleFramePointerUp(event: PointerEvent) {
	if (!isDragging) return;

	isDragging = false;
	(event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
	if (!dragMoved) return;

	event.stopPropagation();
	await onclaim?.({ x: frame.x, y: frame.y });
	onchange?.();
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

	const isSymmetric = event.altKey; // Option/Alt for symmetric resize
	const lockAspect = event.shiftKey || aspectLocked; // Shift for aspect lock, or persistent lock

	let newX = dragStart.frameX;
	let newY = dragStart.frameY;
	let newW = dragStart.frameW;
	let newH = dragStart.frameH;

	if (isSymmetric) {
		// Symmetric resize from center
		const centerX = dragStart.frameX + dragStart.frameW / 2;
		const centerY = dragStart.frameY + dragStart.frameH / 2;

		if (resizeHandle.includes('e')) {
			newW = dragStart.frameW + deltaX * 2;
		} else if (resizeHandle.includes('w')) {
			newW = dragStart.frameW - deltaX * 2;
		}

		if (resizeHandle.includes('s')) {
			newH = dragStart.frameH + deltaY * 2;
		} else if (resizeHandle.includes('n')) {
			newH = dragStart.frameH - deltaY * 2;
		}

		newW = Math.max(0.05, newW);
		newH = Math.max(0.05, newH);

		newX = centerX - newW / 2;
		newY = centerY - newH / 2;
	} else {
		// Standard resize from edge
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
	}

	// Apply aspect ratio lock if Shift is held
	if (lockAspect) {
		const originalAspect = dragStart.frameW / dragStart.frameH;

		if (resizeHandle.includes('e') || resizeHandle.includes('w')) {
			newH = newW / originalAspect;
			if (!isSymmetric && resizeHandle.includes('n')) {
				newY = dragStart.frameY + dragStart.frameH - newH;
			}
		} else if (resizeHandle.includes('n') || resizeHandle.includes('s')) {
			newW = newH * originalAspect;
			if (!isSymmetric && resizeHandle.includes('w')) {
				newX = dragStart.frameX + dragStart.frameW - newW;
			}
		}

		// For corner handles, use the dominant axis
		if (resizeHandle.length === 2) {
			const newAspect = newW / newH;
			if (Math.abs(newAspect - originalAspect) > 0.1) {
				newH = newW / originalAspect;
				if (!isSymmetric) {
					if (resizeHandle.includes('n')) {
						newY = dragStart.frameY + dragStart.frameH - newH;
					}
				}
			}
		}
	}

	// Clamp to image bounds
	newX = Math.max(0, Math.min(1 - newW, newX));
	newY = Math.max(0, Math.min(1 - newH, newY));
	newW = Math.max(0.05, Math.min(1 - newX, newW));
	newH = Math.max(0.05, Math.min(1 - newY, newH));

	frame.x = newX;
	frame.y = newY;
	frame.width = newW;
	frame.height = newH;
}

async function handleHandlePointerUp(event: PointerEvent) {
	if (!isResizing) return;
	event.stopPropagation();

	isResizing = false;
	resizeHandle = null;
	(event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);

	await onclaim?.({ x: frame.x, y: frame.y, width: frame.width, height: frame.height });
	onchange?.();
}
</script>

<div
	class="frame-overlay"
	class:selected
	class:export-checked={exportChecked}
	class:export-unchecked={!exportChecked}
	class:dragging={isDragging}
	class:resizing={isResizing}
	class:clips-required={clipsRequired}
	style="
		left: {frame.x * 100}%;
		top: {frame.y * 100}%;
		width: {frame.width * 100}%;
		height: {frame.height * 100}%;
		border-width: {borderWidth}px;
		{frame.rotation ? `transform: rotate(${frame.rotation}deg);` : ''}
	"
	role="button"
	tabindex="0"
	onpointerdown={handleFramePointerDown}
	onpointermove={handleFramePointerMove}
	onpointerup={handleFramePointerUp}
>
	<div class="frame-label" class:auto-label={isAuto} class:clip-label={clipsRequired} style="transform: scale({labelScale}); transform-origin: bottom left;">
		{isAuto ? frame.label : index + 1}{#if clipsRequired}<span class="clip-warning" title="Crops out a required focal point">&#9888;</span>{/if}{#if compositionLabel}<span class="comp-hint">{compositionLabel}</span>{/if}
	</div>

	<!-- Resize handles -->
	<div class="handle nw" style="{hs}top:{handleOffset}px;left:{handleOffset}px;" role="button" tabindex="-1" aria-label="Resize northwest" onpointerdown={(e) => handleHandlePointerDown(e, 'nw')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
	<div class="handle ne" style="{hs}top:{handleOffset}px;right:{handleOffset}px;" role="button" tabindex="-1" aria-label="Resize northeast" onpointerdown={(e) => handleHandlePointerDown(e, 'ne')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
	<div class="handle sw" style="{hs}bottom:{handleOffset}px;left:{handleOffset}px;" role="button" tabindex="-1" aria-label="Resize southwest" onpointerdown={(e) => handleHandlePointerDown(e, 'sw')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
	<div class="handle se" style="{hs}bottom:{handleOffset}px;right:{handleOffset}px;" role="button" tabindex="-1" aria-label="Resize southeast" onpointerdown={(e) => handleHandlePointerDown(e, 'se')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
	<div class="handle n" style="{hs}top:{handleOffset}px;left:50%;transform:translateX(-50%);" role="button" tabindex="-1" aria-label="Resize north" onpointerdown={(e) => handleHandlePointerDown(e, 'n')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
	<div class="handle s" style="{hs}bottom:{handleOffset}px;left:50%;transform:translateX(-50%);" role="button" tabindex="-1" aria-label="Resize south" onpointerdown={(e) => handleHandlePointerDown(e, 's')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
	<div class="handle w" style="{hs}top:50%;left:{handleOffset}px;transform:translateY(-50%);" role="button" tabindex="-1" aria-label="Resize west" onpointerdown={(e) => handleHandlePointerDown(e, 'w')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
	<div class="handle e" style="{hs}top:50%;right:{handleOffset}px;transform:translateY(-50%);" role="button" tabindex="-1" aria-label="Resize east" onpointerdown={(e) => handleHandlePointerDown(e, 'e')} onpointermove={handleHandlePointerMove} onpointerup={handleHandlePointerUp}></div>
</div>

<style>
	.frame-overlay {
		position: absolute;
		border-style: solid;
		border-color: #3b82f6;
		box-sizing: border-box;
		cursor: move;
		pointer-events: none;
		transition: left 0.3s ease, top 0.3s ease, width 0.3s ease, height 0.3s ease;
	}

	/* Only the selected frame takes pointer events; the rest would bury each other and
	   swallow every click. The canvas hit-tests the others against their geometry. */
	.frame-overlay.selected,
	.frame-overlay.selected .handle {
		pointer-events: auto;
	}

	/* Export-unchecked: grey */
	.frame-overlay.export-unchecked {
		border-color: #9ca3af;
		opacity: 0.5;
	}

	/* Export-checked: blue */
	.frame-overlay.export-checked {
		border-color: #3b82f6;
	}

	.frame-overlay:hover {
		z-index: 5;
	}

	/* Selected for editing: orange (overrides blue/grey) */
	.frame-overlay.selected {
		border-color: #f59e0b;
		z-index: 10;
		opacity: 1;
	}

	/* Crops out a required focal point */
	.frame-overlay.clips-required {
		border-color: #f97316;
		box-shadow: 0 0 0 1px rgba(239, 68, 68, 0.5), inset 0 0 24px rgba(239, 68, 68, 0.15);
	}

	.frame-label.clip-label {
		background: #dc2626;
	}

	.clip-warning {
		margin-left: 0.4em;
	}

	.frame-overlay.dragging,
	.frame-overlay.resizing {
		border-color: #f59e0b;
		z-index: 10;
		opacity: 1;
		transition: none;
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

	.frame-overlay.export-unchecked .frame-label {
		background: #9ca3af;
	}

	.frame-overlay.selected .frame-label {
		background: #f59e0b;
	}

	.frame-label.auto-label {
		background: #0891b2;
	}

	.frame-overlay.selected .frame-label.auto-label {
		background: #f59e0b;
	}

	.comp-hint {
		margin-left: 0.4em;
		opacity: 0.7;
		font-size: 0.65rem;
		font-weight: 400;
	}

	.handle {
		position: absolute;
		background: white;
		border-style: solid;
		border-color: #3b82f6;
		box-sizing: border-box;
	}

	.frame-overlay.export-unchecked .handle {
		border-color: #9ca3af;
	}

	.frame-overlay.selected .handle {
		border-color: #f59e0b;
	}

	.handle:hover {
		background: #3b82f6;
	}

	.frame-overlay.selected .handle:hover {
		background: #f59e0b;
	}

	.handle.nw { cursor: nw-resize; }
	.handle.ne { cursor: ne-resize; }
	.handle.sw { cursor: sw-resize; }
	.handle.se { cursor: se-resize; }
	.handle.n { cursor: n-resize; }
	.handle.s { cursor: s-resize; }
	.handle.w { cursor: w-resize; }
	.handle.e { cursor: e-resize; }
</style>
