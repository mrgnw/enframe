<script lang="ts">
// The start screens' visual, 132 x 92, in the app's brand colour. `frame` is enframe's: the scene holds
// and the crop marks step between eight framings, keeping two ghosts of the framings before. `hold` is
// Stabilize's: the point on the moon holds and the photo flips ten pages a second around it, with two
// ghosts of the pages just turned. Every move is a cut (step-end), a page of a flip book.
let { kind }: { kind: 'frame' | 'hold' } = $props();
</script>

{#snippet scene()}
	<svg class="scene" viewBox="0 0 132 92" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
		<path d="M0 70 C 20 60, 34 64, 50 56 S 86 52, 104 60 S 124 66, 132 62 V 92 H 0 Z" fill="#3f3f46" />
		<path d="M0 80 C 30 72, 60 78, 90 74 S 120 70, 132 76 V 92 H 0 Z" fill="#27272a" />
		<circle cx="66" cy="44" r="7" fill="#71717a" />
	</svg>
{/snippet}

{#snippet marks()}
	<span class="mark tl"></span><span class="mark tr"></span><span class="mark bl"></span><span class="mark br"></span>
{/snippet}

<div class="visual {kind}" aria-hidden="true">
	{#if kind === 'frame'}
		{@render scene()}
		<div class="crop ghost2">{@render marks()}</div>
		<div class="crop ghost">{@render marks()}</div>
		<div class="crop live">{@render marks()}</div>
	{:else}
		<span class="edge ghost2"></span>
		<span class="edge ghost"></span>
		<div class="photo">{@render scene()}</div>
		<span class="dot"></span>
	{/if}
</div>

<style>
	.visual {
		position: relative;
		width: 132px;
		height: 92px;
		overflow: hidden;
		--crop-loop: 4s;
		--crop-step: 0.5s;
		--flip-loop: 3s;
		--flip-step: 0.1s;
	}
	.scene {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		opacity: 0.55;
	}
	.crop,
	.photo,
	.photo::after,
	.edge {
		animation-timing-function: step-end;
		animation-iteration-count: infinite;
	}

	.crop {
		position: absolute;
		animation-name: crops;
		animation-duration: var(--crop-loop);
	}
	.crop.live {
		box-shadow: 0 0 0 200px rgba(26, 26, 26, 0.55);
	}
	.crop.ghost {
		opacity: 0.35;
		animation-delay: calc(var(--crop-step) - var(--crop-loop));
	}
	.crop.ghost2 {
		opacity: 0.15;
		animation-delay: calc(2 * var(--crop-step) - var(--crop-loop));
	}
	.mark {
		position: absolute;
		width: 22px;
		height: 22px;
		border: 2px solid var(--brand);
	}
	.tl { top: 0; left: 0; border-right: none; border-bottom: none; }
	.tr { top: 0; right: 0; border-left: none; border-bottom: none; }
	.bl { bottom: 0; left: 0; border-right: none; border-top: none; }
	.br { bottom: 0; right: 0; border-left: none; border-top: none; }
	@keyframes crops {
		0% { inset: 8px 44px 8px 6px; }
		12.5% { inset: 4px 10px 4px 68px; }
		25% { inset: 22px 24px 28px 22px; }
		37.5% { inset: 0 56px 0 0; }
		50% { inset: 34px 6px 6px 44px; }
		62.5% { inset: 10px 10px 10px 10px; }
		75% { inset: 0 0 40px 30px; }
		87.5% { inset: 26px 50px 0 10px; }
	}

	.photo {
		position: absolute;
		inset: 0;
		animation-name: photo-clip;
		animation-duration: var(--flip-loop);
	}
	.photo::after,
	.edge {
		content: '';
		position: absolute;
		inset: 0;
		border: 2px solid #71717a;
		animation-name: photo-edge;
		animation-duration: var(--flip-loop);
	}
	.edge {
		border-width: 1.5px;
	}
	.edge.ghost {
		opacity: 0.4;
		animation-delay: calc(var(--flip-step) - var(--flip-loop));
	}
	.edge.ghost2 {
		opacity: 0.18;
		animation-delay: calc(2 * var(--flip-step) - var(--flip-loop));
	}
	.dot {
		position: absolute;
		left: 66px;
		top: 44px;
		width: 7px;
		height: 7px;
		margin: -3.5px 0 0 -3.5px;
		border-radius: 999px;
		background: var(--brand);
		box-shadow: 0 0 0 1.5px var(--bg);
	}
	@keyframes photo-clip {
		0% { clip-path: inset(6.5px 21.3px 13.5px 14.7px); }
		3.333% { clip-path: inset(4.3px 16.5px 15.7px 19.5px); }
		6.667% { clip-path: inset(2.8px 16.4px 17.2px 19.6px); }
		10% { clip-path: inset(7.5px 17.5px 12.5px 18.5px); }
		13.333% { clip-path: inset(7.0px 23.1px 13.0px 12.9px); }
		16.667% { clip-path: inset(5.5px 26.9px 14.5px 9.1px); }
		20% { clip-path: inset(3.8px 20.0px 16.2px 16.0px); }
		23.333% { clip-path: inset(3.0px 18.4px 17.0px 17.6px); }
		26.667% { clip-path: inset(2.4px 15.2px 17.6px 20.8px); }
		30% { clip-path: inset(1.8px 19.8px 18.2px 16.2px); }
		33.333% { clip-path: inset(4.7px 21.1px 15.3px 14.9px); }
		36.667% { clip-path: inset(1.7px 20.2px 18.3px 15.8px); }
		40% { clip-path: inset(0px 17.8px 20px 18.2px); }
		43.333% { clip-path: inset(0px 17.6px 20px 18.4px); }
		46.667% { clip-path: inset(2.9px 18.9px 17.1px 17.1px); }
		50% { clip-path: inset(5.4px 18.7px 14.6px 17.3px); }
		53.333% { clip-path: inset(6.3px 15.1px 13.7px 20.9px); }
		56.667% { clip-path: inset(5.2px 13.0px 14.8px 23.0px); }
		60% { clip-path: inset(8.0px 11.4px 12.0px 24.6px); }
		63.333% { clip-path: inset(12.4px 17.0px 7.6px 19.0px); }
		66.667% { clip-path: inset(8.8px 18.9px 11.2px 17.1px); }
		70% { clip-path: inset(7.2px 23.8px 12.8px 12.2px); }
		73.333% { clip-path: inset(6.2px 29.8px 13.8px 6.2px); }
		76.667% { clip-path: inset(7.7px 31.0px 12.3px 5.0px); }
		80% { clip-path: inset(12.1px 31.5px 7.9px 4.5px); }
		83.333% { clip-path: inset(10.3px 29.2px 9.7px 6.8px); }
		86.667% { clip-path: inset(12.3px 24.0px 7.7px 12.0px); }
		90% { clip-path: inset(14.6px 17.4px 5.4px 18.6px); }
		93.333% { clip-path: inset(11.7px 10.9px 8.3px 25.1px); }
		96.667% { clip-path: inset(16.4px 14.0px 3.6px 22.0px); }
	}
	@keyframes photo-edge {
		0% { inset: 6.5px 21.3px 13.5px 14.7px; }
		3.333% { inset: 4.3px 16.5px 15.7px 19.5px; }
		6.667% { inset: 2.8px 16.4px 17.2px 19.6px; }
		10% { inset: 7.5px 17.5px 12.5px 18.5px; }
		13.333% { inset: 7.0px 23.1px 13.0px 12.9px; }
		16.667% { inset: 5.5px 26.9px 14.5px 9.1px; }
		20% { inset: 3.8px 20.0px 16.2px 16.0px; }
		23.333% { inset: 3.0px 18.4px 17.0px 17.6px; }
		26.667% { inset: 2.4px 15.2px 17.6px 20.8px; }
		30% { inset: 1.8px 19.8px 18.2px 16.2px; }
		33.333% { inset: 4.7px 21.1px 15.3px 14.9px; }
		36.667% { inset: 1.7px 20.2px 18.3px 15.8px; }
		40% { inset: 0px 17.8px 20px 18.2px; }
		43.333% { inset: 0px 17.6px 20px 18.4px; }
		46.667% { inset: 2.9px 18.9px 17.1px 17.1px; }
		50% { inset: 5.4px 18.7px 14.6px 17.3px; }
		53.333% { inset: 6.3px 15.1px 13.7px 20.9px; }
		56.667% { inset: 5.2px 13.0px 14.8px 23.0px; }
		60% { inset: 8.0px 11.4px 12.0px 24.6px; }
		63.333% { inset: 12.4px 17.0px 7.6px 19.0px; }
		66.667% { inset: 8.8px 18.9px 11.2px 17.1px; }
		70% { inset: 7.2px 23.8px 12.8px 12.2px; }
		73.333% { inset: 6.2px 29.8px 13.8px 6.2px; }
		76.667% { inset: 7.7px 31.0px 12.3px 5.0px; }
		80% { inset: 12.1px 31.5px 7.9px 4.5px; }
		83.333% { inset: 10.3px 29.2px 9.7px 6.8px; }
		86.667% { inset: 12.3px 24.0px 7.7px 12.0px; }
		90% { inset: 14.6px 17.4px 5.4px 18.6px; }
		93.333% { inset: 11.7px 10.9px 8.3px 25.1px; }
		96.667% { inset: 16.4px 14.0px 3.6px 22.0px; }
	}

	@media (prefers-reduced-motion: reduce) {
		.crop,
		.photo,
		.photo::after,
		.edge {
			animation: none;
		}
		.crop.ghost,
		.crop.ghost2,
		.edge.ghost,
		.edge.ghost2 {
			display: none;
		}
	}
</style>
