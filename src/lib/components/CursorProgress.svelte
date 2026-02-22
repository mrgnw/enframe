<script lang="ts">
interface Props {
	progress: number;
	visible: boolean;
	x: number;
	y: number;
	color?: string;
}

let { progress, visible, x, y, color = '#3b82f6' }: Props = $props();

const size = 40;
const stroke = 3.5;
const radius = (size - stroke) / 2;
const circumference = 2 * Math.PI * radius;
let offset = $derived(circumference * (1 - progress));
</script>

{#if visible}
	<div
		class="cursor-progress"
		style="left:{x}px;top:{y}px;"
	>
		<svg width={size} height={size} viewBox="0 0 {size} {size}">
			<circle
				cx={size / 2}
				cy={size / 2}
				r={radius}
				fill="none"
				stroke="rgba(255,255,255,0.15)"
				stroke-width={stroke}
			/>
			<circle
				cx={size / 2}
				cy={size / 2}
				r={radius}
				fill="none"
				stroke={color}
				stroke-width={stroke}
				stroke-dasharray={circumference}
				stroke-dashoffset={offset}
				stroke-linecap="round"
				transform="rotate(-90 {size / 2} {size / 2})"
				class="arc"
			/>
		</svg>
	</div>
{/if}

<style>
	.cursor-progress {
		position: fixed;
		z-index: 200;
		pointer-events: none;
		transform: translate(-50%, -50%);
		filter: drop-shadow(0 1px 4px rgba(0, 0, 0, 0.5));
	}

	.arc {
		transition: stroke-dashoffset 0.15s ease;
	}
</style>
