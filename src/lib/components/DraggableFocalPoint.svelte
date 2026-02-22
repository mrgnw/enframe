<script lang="ts">
import type { FocalPoint } from '$lib/db';
import { updateFocalPoint } from '$lib/db';

interface Props {
	point: FocalPoint;
	imageWidth: number;
	imageHeight: number;
	scale: number;
}

let { point, imageWidth, imageHeight, scale }: Props = $props();

let isDragging = $state(false);
let dragStart = $state({ x: 0, y: 0, pointX: 0, pointY: 0 });

function handlePointerDown(event: PointerEvent) {
	if (event.button !== 0) return;
	event.stopPropagation();

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

	await updateFocalPoint(point.id, { x: point.x, y: point.y });
}
</script>

<div
	class="focal-point"
	class:required={point.required}
	class:dragging={isDragging}
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
	{#if point.label}
		<div class="point-label">{point.label}</div>
	{/if}
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
		transition: transform 0.1s;
	}

	.focal-point:hover {
		transform: scale(1.2);
		z-index: 10;
	}

	.focal-point.dragging {
		transform: scale(1.3);
		z-index: 20;
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
