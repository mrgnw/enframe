<script lang="ts">
const PHI = 0.618033988749895;
const PHI_INV = 1 - PHI;

interface Props {
	guide: 'off' | 'thirds' | 'phi' | 'spiral' | 'triangle' | 'diagonal';
	width: number;
	height: number;
}

let { guide, width, height }: Props = $props();

function buildSpiralPath(w: number, h: number): string {
	// Fibonacci-style golden spiral: quarter arcs through subdivided golden rectangles
	const pts: string[] = [];
	let rx = 0,
		ry = 0,
		rw = w,
		rh = h;

	for (let i = 0; i < 7; i++) {
		const step = i % 4;
		let cx: number, cy: number, r: number, startAngle: number;

		if (step === 0) {
			r = rh * PHI_INV;
			cx = rx + rw * PHI;
			cy = ry + rh;
			startAngle = Math.PI;
			const newRh = rh * PHI_INV;
			ry = ry + rh - newRh;
			rh = newRh;
		} else if (step === 1) {
			r = rw * PHI_INV;
			cx = rx;
			cy = ry + rh * PHI;
			startAngle = Math.PI * 1.5;
			const newRw = rw * PHI_INV;
			rw = newRw;
		} else if (step === 2) {
			r = rh * PHI_INV;
			cx = rx + rw * PHI_INV;
			cy = ry;
			startAngle = 0;
			const newRh = rh * PHI_INV;
			rh = newRh;
		} else {
			r = rw * PHI_INV;
			cx = rx + rw;
			cy = ry + rh * PHI_INV;
			startAngle = Math.PI * 0.5;
			const newRw = rw * PHI_INV;
			rx = rx + rw - newRw;
			rw = newRw;
		}

		const endAngle = startAngle + Math.PI / 2;
		const x1 = cx + r * Math.cos(startAngle);
		const y1 = cy + r * Math.sin(startAngle);
		const x2 = cx + r * Math.cos(endAngle);
		const y2 = cy + r * Math.sin(endAngle);

		if (i === 0) pts.push(`M ${x1.toFixed(2)} ${y1.toFixed(2)}`);
		pts.push(`A ${r.toFixed(2)} ${r.toFixed(2)} 0 0 1 ${x2.toFixed(2)} ${y2.toFixed(2)}`);
	}

	return pts.join(' ');
}
</script>

{#if guide !== 'off' && width > 0 && height > 0}
	<svg class="guide-overlay" viewBox="0 0 {width} {height}" xmlns="http://www.w3.org/2000/svg">
		{#if guide === 'thirds'}
			<line x1={width / 3} y1={0} x2={width / 3} y2={height} />
			<line x1={(width * 2) / 3} y1={0} x2={(width * 2) / 3} y2={height} />
			<line x1={0} y1={height / 3} x2={width} y2={height / 3} />
			<line x1={0} y1={(height * 2) / 3} x2={width} y2={(height * 2) / 3} />
			<circle cx={width / 3} cy={height / 3} r={3} />
			<circle cx={(width * 2) / 3} cy={height / 3} r={3} />
			<circle cx={width / 3} cy={(height * 2) / 3} r={3} />
			<circle cx={(width * 2) / 3} cy={(height * 2) / 3} r={3} />
		{:else if guide === 'phi'}
			<line x1={width * PHI_INV} y1={0} x2={width * PHI_INV} y2={height} />
			<line x1={width * PHI} y1={0} x2={width * PHI} y2={height} />
			<line x1={0} y1={height * PHI_INV} x2={width} y2={height * PHI_INV} />
			<line x1={0} y1={height * PHI} x2={width} y2={height * PHI} />
			<circle cx={width * PHI_INV} cy={height * PHI_INV} r={3} />
			<circle cx={width * PHI} cy={height * PHI_INV} r={3} />
			<circle cx={width * PHI_INV} cy={height * PHI} r={3} />
			<circle cx={width * PHI} cy={height * PHI} r={3} />
		{:else if guide === 'spiral'}
			<path d={buildSpiralPath(width, height)} fill="none" />
		{:else if guide === 'triangle'}
			<line x1={0} y1={height} x2={width} y2={0} />
			<line x1={0} y1={0} x2={width * PHI_INV} y2={height * PHI} />
			<line x1={width} y1={height} x2={width * PHI} y2={height * PHI_INV} />
		{:else if guide === 'diagonal'}
			{@const sq = Math.min(width, height)}
			{@const offX = (width - sq) / 2}
			{@const offY = (height - sq) / 2}
			<line x1={offX} y1={offY} x2={offX + sq} y2={offY + sq} />
			<line x1={offX + sq} y1={offY} x2={offX} y2={offY + sq} />
		{/if}
	</svg>
{/if}

<style>
	.guide-overlay {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		pointer-events: none;
	}

	.guide-overlay line {
		stroke: rgba(255, 255, 255, 0.3);
		stroke-width: 1;
		vector-effect: non-scaling-stroke;
	}

	.guide-overlay circle {
		fill: rgba(255, 255, 255, 0.5);
	}

	.guide-overlay path {
		stroke: rgba(255, 255, 255, 0.3);
		stroke-width: 1;
		vector-effect: non-scaling-stroke;
	}
</style>
