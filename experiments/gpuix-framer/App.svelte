<script lang="ts">
import { onMount } from 'svelte';
import { on_window_key, zoom_window, type GpuixEvent } from 'gpuix-svelte';
import { crop, fit, toFocal, type Rect } from './geometry.ts';

let { photo, photoWidth, photoHeight }: { photo: string; photoWidth: number; photoHeight: number } = $props();

const PREVIEW = 180;
const CROP_SIDE = $derived(photoHeight / 2);

let stage = $state<Rect>({ x: 0, y: 0, width: 0, height: 0 });
let focal = $state({ x: 0.5, y: 0.5 });
let dragging = $state(false);
let moves = $state(0);

const shown = $derived(fit(stage, photoWidth, photoHeight));
const cropped = $derived(crop(focal, photoWidth, photoHeight, CROP_SIDE));
const previewScale = $derived(PREVIEW / CROP_SIDE);

onMount(() =>
	on_window_key('keydown', (e) => {
		if (e.key === 'z') zoom_window();
	})
);

function onlayout(e: GpuixEvent) {
	stage = { x: e.x!, y: e.y!, width: e.width!, height: e.height! };
}

function grab(e: GpuixEvent) {
	if (e.button !== 0) return;
	dragging = true;
	focal = toFocal(shown, e.x!, e.y!);
}

function move(e: GpuixEvent) {
	if (process.env.FRAMER_LOG) console.error(`move ${e.x},${e.y} pressed=${e.pressedButton}`);
	if (!dragging) return;
	if (e.pressedButton == null) {
		dragging = false;
		return;
	}
	moves++;
	focal = toFocal(shown, e.x!, e.y!);
}

function release() {
	if (process.env.FRAMER_LOG) console.error('up');
	dragging = false;
}
</script>

<div class="app">
	<div class="stage" testId="stage" {onlayout} onmousedown={grab} onmousemove={move} onmouseup={release}>
		<div class="photo-frame" style="aspect-ratio: {photoWidth / photoHeight};">
			<img class="photo stage-photo" testId="photo" src={photo} />
			<div class="marker-anchor" style="width: {focal.x * 100}%; height: {focal.y * 100}%;">
				<div class="marker" class:dragging testId="marker"></div>
			</div>
		</div>
	</div>
	<div class="side">
		<text class="label">Crop preview 1:1</text>
		<div class="preview" testId="preview">
			<img
				class="photo"
				src={photo}
				style="left: {-cropped.x * previewScale}px; top: {-cropped.y * previewScale}px; width: {photoWidth *
					previewScale}px; height: {photoHeight * previewScale}px;"
			/>
		</div>
		<text class="label" testId="readout">focal {focal.x.toFixed(3)}, {focal.y.toFixed(3)}</text>
		<text class="label" testId="crop">crop {Math.round(cropped.x)}, {Math.round(cropped.y)}</text>
		<text class="label" testId="state">{dragging ? 'dragging' : 'idle'} · moves {moves}</text>
		<text class="label" testId="size">stage {Math.round(stage.width)}×{Math.round(stage.height)}</text>
	</div>
</div>

<style>
	.app {
		display: flex;
		flex-direction: row;
		width: 100%;
		height: 100%;
		background-color: #111111;
	}
	.stage {
		display: flex;
		align-items: center;
		justify-content: center;
		position: relative;
		flex-grow: 1;
		height: 100%;
		overflow: hidden;
		cursor: crosshair;
	}
	.photo {
		position: absolute;
		pointer-events: none;
	}
	.photo-frame {
		position: relative;
		width: 100%;
		max-height: 100%;
		pointer-events: none;
	}
	.stage-photo {
		left: 0;
		top: 0;
		width: 100%;
		height: 100%;
	}
	.marker-anchor {
		position: relative;
		pointer-events: none;
	}
	.marker {
		position: absolute;
		right: -10px;
		bottom: -10px;
		width: 20px;
		height: 20px;
		border-radius: 10px;
		border-width: 3px;
		border-style: solid;
		border-color: #ffffff;
		background-color: #e5484d;
		pointer-events: none;
	}
	.marker.dragging {
		background-color: #30a46c;
	}
	.side {
		user-select: none;
		display: flex;
		flex-direction: column;
		gap: 8px;
		width: 220px;
		padding: 20px;
	}
	.preview {
		position: relative;
		width: 180px;
		height: 180px;
		overflow: hidden;
		border-width: 1px;
		border-style: solid;
		border-color: #444444;
	}
	.label {
		color: #dddddd;
		font-size: 13px;
	}
</style>
