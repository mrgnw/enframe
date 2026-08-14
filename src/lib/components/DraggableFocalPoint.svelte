<script lang="ts">
import type { FocalPoint, SelectMode } from '$lib/framer/types';

interface Props {
	point: FocalPoint;
	index?: number;
	imageWidth: number;
	imageHeight: number;
	scale: number;
	selected?: boolean;
	inSelection?: boolean;
	onSelect?: (mode: SelectMode) => void;
	onmove?: (position: { x: number; y: number }) => Promise<void> | void;
	onchange?: () => void;
}

let {
	point,
	index = 0,
	imageWidth,
	imageHeight,
	scale,
	selected = false,
	inSelection = false,
	onSelect,
	onmove,
	onchange
}: Props = $props();

let isDragging = $state(false);
let dragStart = $state({ x: 0, y: 0, pointX: 0, pointY: 0 });

function handlePointerDown(event: PointerEvent) {
	if (event.button !== 0) return;
	event.stopPropagation();

	onSelect?.(event.shiftKey || event.metaKey ? 'toggle' : 'replace');
	isDragging = true;
	dragStart = {
		x: event.clientX,
		y: event.clientY,
		pointX: point.x,
		pointY: point.y
	};
	(event.target as HTMLElement).setPointerCapture(event.pointerId);
}

function handlePointerMove(event: PointerEvent) {
	if (!isDragging) return;
	event.stopPropagation();

	const deltaX = (event.clientX - dragStart.x) / scale / imageWidth;
	const deltaY = (event.clientY - dragStart.y) / scale / imageHeight;

	const newX = Math.max(0, Math.min(1, dragStart.pointX + deltaX));
	const newY = Math.max(0, Math.min(1, dragStart.pointY + deltaY));

	point.x = newX;
	point.y = newY;
}

async function handlePointerUp(event: PointerEvent) {
	if (!isDragging) return;
	event.stopPropagation();

	isDragging = false;
	(event.target as HTMLElement).releasePointerCapture(event.pointerId);

	await onmove?.({ x: point.x, y: point.y });
	onchange?.();
}
</script>

<div
	class="focal-point"
	class:required={point.required}
	class:dragging={isDragging}
	class:selected
	class:in-selection={inSelection}
	style="
		left: {point.x * 100}%;
		top: {point.y * 100}%;
	"
	title={point.label || 'Focal point'}
	role="button"
	tabindex="0"
	onpointerdown={handlePointerDown}
	onpointermove={handlePointerMove}
	onpointerup={handlePointerUp}
>
	<div class="point-label">•{index + 1}</div>
</div>

<style>
	.focal-point {
		position: absolute;
		width: 16px;
		height: 16px;
		margin: -8px 0 0 -8px;
		background: #ef4444;
		border: 2px solid white;
		border-radius: 50%;
		box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
		cursor: move;
		pointer-events: auto;
		/* Above the selected frame (z-index 10) so a point inside a frame stays clickable. */
		z-index: 20;
		transition: transform 0.1s;
	}

	.focal-point:hover {
		transform: scale(1.2);
		z-index: 25;
	}

	.focal-point.in-selection {
		box-shadow: 0 0 0 3px rgba(239, 68, 68, 0.4), 0 2px 4px rgba(0, 0, 0, 0.2);
	}

	.focal-point.selected {
		transform: scale(1.3);
		z-index: 30;
		box-shadow: 0 0 0 3px rgba(239, 68, 68, 0.4), 0 4px 8px rgba(0, 0, 0, 0.3);
	}

	.focal-point.dragging {
		transform: scale(1.3);
		z-index: 35;
		box-shadow: 0 4px 8px rgba(0, 0, 0, 0.3);
	}

	.focal-point.required {
		background: #dc2626;
		border-width: 3px;
	}

	.point-label {
		position: absolute;
		top: -2rem;
		left: 50%;
		transform: translateX(-50%);
		background: #ef4444;
		color: white;
		padding: 0.25rem 0.5rem;
		font-size: 0.75rem;
		border-radius: 0.25rem;
		white-space: nowrap;
		pointer-events: none;
	}
</style>
