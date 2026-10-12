# /// script
# requires-python = ">=3.12"
# ///
# Writes mockups/stabilize/04-start-story.html. The flip-book keyframes are generated here
# so the page can have many small steps without hand-typing them.
import random
from pathlib import Path

OUT = Path(__file__).resolve().parents[2] / 'mockups/stabilize/04-start-story.html'
FRAMES = 57
CLIP_FPS = 8
SKETCH_FPS = 10
SKETCH_STEPS = 30
CROP_FPS = 2

# Stabilize A: a 96 x 72 photo wandering inside 132 x 92 so the moon at (66, 44) stays inside it.
rng = random.Random(29)
left, top = 14.0, 8.0
sketch = []
for _ in range(SKETCH_STEPS):
	left = min(36, max(0, left + rng.uniform(-7, 7)))
	top = min(20, max(0, top + rng.uniform(-5, 5)))
	sketch.append((round(top, 1), round(132 - 96 - left, 1), round(92 - 72 - top, 1), round(left, 1)))

# enframe: eight framings of the same image, as inset t r b l in the 132 x 92 footprint
crops = [
	'8px 44px 8px 6px',
	'4px 10px 4px 68px',
	'22px 24px 28px 22px',
	'0 56px 0 0',
	'34px 6px 6px 44px',
	'10px 10px 10px 10px',
	'0 0 40px 30px',
	'26px 50px 0 10px',
]


def steps(name: str, prop: str, values: list[str]) -> str:
	n = len(values)
	lines = [f'@keyframes {name} {{']
	for i, v in enumerate(values):
		pct = f'{100 * i / n:.3f}'.rstrip('0').rstrip('.')
		lines.append(f'\t\t{pct}% {{ {prop}: {v}; }}')
	lines.append('\t}')
	return '\n'.join(lines)


sketch_clip = steps('sketch-clip', 'clip-path', [f'inset({t}px {r}px {b}px {l}px)' for t, r, b, l in sketch])
sketch_edge = steps('sketch-edge', 'inset', [f'{t}px {r}px {b}px {l}px' for t, r, b, l in sketch])
crop_frames = steps('crops', 'inset', crops)
page_pct = f'{100 / FRAMES:.4f}'

clip_imgs = '\n'.join(
	f'\t\t\t\t<img src="assets/frame-{i + 1:04d}.webp" alt="" style="--i:{i}">' for i in range(FRAMES)
)

html = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Start screens · one image, many stories · many images, one thing</title>
<style>
	:root {{
		color-scheme: dark;
		--blue: #2563eb;
		--amber: #f5a524;
		--mark: #52525b;
		--screen: #1a1a1a;
		--sketch-loop: {SKETCH_STEPS / SKETCH_FPS}s;
		--sketch-step: {1 / SKETCH_FPS}s;
		--clip-loop: {FRAMES / CLIP_FPS}s;
		--crop-loop: {len(crops) / CROP_FPS}s;
		--crop-step: {1 / CROP_FPS}s;
	}}
	* {{ box-sizing: border-box; }}
	body {{
		margin: 0;
		background: #111113;
		color: #a1a1aa;
		font-family: ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji";
	}}

	.note {{
		display: flex;
		flex-wrap: wrap;
		align-items: end;
		justify-content: space-between;
		gap: 0.75rem 1.5rem;
		max-width: 1120px;
		margin: 0 auto;
		padding: 28px 24px 4px;
	}}
	.note h1 {{ margin: 0 0 4px; color: #f4f4f5; font-size: 1.1rem; font-weight: 600; letter-spacing: -0.01em; }}
	.note p {{ margin: 0; max-width: 72ch; font-size: 0.85rem; line-height: 1.55; }}
	.note label {{ display: inline-flex; align-items: center; gap: 0.4rem; font-size: 0.8rem; cursor: pointer; }}
	.note input {{ accent-color: var(--amber); }}

	.row {{ max-width: 1120px; margin: 0 auto; padding: 18px 24px 8px; }}
	.row h2 {{ margin: 0 0 10px; color: #e4e4e7; font-size: 0.9rem; font-weight: 600; }}
	.row h2 span {{ color: #71717a; font-weight: 400; }}
	.pair {{ display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; }}
	@media (max-width: 820px) {{ .pair {{ grid-template-columns: 1fr; }} }}

	/* The start screen, as Standalone.svelte's .empty and Stabilize's .drop */
	.screen {{
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 400px;
		padding: 0 2rem;
		text-align: center;
		background: var(--screen);
		border: 1px solid #27272a;
		border-radius: 12px;
	}}
	.screen h1 {{ margin: 0 0 0.75rem; color: #f4f4f5; font-size: 1.65rem; font-weight: 600; letter-spacing: -0.02em; line-height: 1.2; }}
	.screen h1 b {{ font-weight: inherit; }}
	.enframe h1 b {{ color: var(--blue); }}
	.stabilize h1 b {{ color: var(--amber); }}
	.lede {{ max-width: 42ch; margin: 0 0 1.75rem; color: #a1a1aa; font-size: 0.9rem; line-height: 1.65; }}
	.open {{
		padding: 0.55rem 1.15rem;
		color: white;
		font: inherit;
		font-size: 0.85rem;
		font-weight: 500;
		background: var(--blue);
		border: none;
		border-radius: 0.5rem;
		cursor: pointer;
	}}
	.stabilize .open {{ color: #1a1200; background: var(--amber); }}

	/* One footprint for both: enframe's 132 × 92 */
	.visual {{ position: relative; width: 132px; height: 92px; margin-bottom: 2.5rem; overflow: hidden; }}
	.mark {{ position: absolute; width: 22px; height: 22px; border: 2px solid var(--mark); }}
	.tl {{ top: 0; left: 0; border-right: none; border-bottom: none; }}
	.tr {{ top: 0; right: 0; border-left: none; border-bottom: none; }}
	.bl {{ bottom: 0; left: 0; border-right: none; border-top: none; }}
	.br {{ bottom: 0; right: 0; border-left: none; border-top: none; }}
	.dot {{
		position: absolute;
		width: 7px;
		height: 7px;
		margin: -3.5px 0 0 -3.5px;
		border-radius: 999px;
		background: var(--amber);
		box-shadow: 0 0 0 1.5px var(--screen);
	}}
	.scene {{ position: absolute; inset: 0; width: 100%; height: 100%; }}

	/* Every move is a cut: step-end holds each keyframe until the next, like a page of a flip book. */
	.visual .flip, .visual .flip::after {{ animation-timing-function: step-end; animation-iteration-count: infinite; }}

	/* ── enframe: the image holds, the crop cuts between eight framings, {CROP_FPS} a second.
	   Two ghosts hold the previous framings, the stories already told. */
	.enframe .crop {{ position: absolute; animation-name: crops; animation-duration: var(--crop-loop); }}
	.enframe .crop.ghost {{ opacity: 0.35; animation-delay: calc(var(--crop-step) - var(--crop-loop)); }}
	.enframe .crop.ghost2 {{ opacity: 0.15; animation-delay: calc(2 * var(--crop-step) - var(--crop-loop)); }}
	.enframe .crop.live {{ box-shadow: 0 0 0 200px rgba(26, 26, 26, 0.55); }}
	.a.enframe .crop.live .mark {{ border-color: var(--blue); }}
	.a.enframe .crop.ghost .mark, .a.enframe .crop.ghost2 .mark {{ border-color: var(--blue); }}
	.b.enframe .crop .mark {{ border-color: #fff; }}
	.b.enframe .crop.live {{ box-shadow: 0 0 0 200px rgba(0, 0, 0, 0.6); }}
	{crop_frames}

	/* ── A · Sketch: the moon holds under the point; the photo flips {SKETCH_FPS} pages a second around it,
	   {SKETCH_STEPS} pages, each a little off the last. Two ghosts show the pages just turned. */
	.a .scene {{ opacity: 0.55; }}
	.a.stabilize .photo {{ position: absolute; inset: 0; animation-name: sketch-clip; animation-duration: var(--sketch-loop); }}
	.a.stabilize .photo::after {{ content: ""; position: absolute; inset: 0; border: 2px solid #71717a; animation-name: sketch-edge; animation-duration: var(--sketch-loop); }}
	.a.stabilize .edge {{ position: absolute; border: 1.5px solid #71717a; animation-name: sketch-edge; animation-duration: var(--sketch-loop); }}
	.a.stabilize .edge.ghost {{ opacity: 0.4; animation-delay: calc(var(--sketch-step) - var(--sketch-loop)); }}
	.a.stabilize .edge.ghost2 {{ opacity: 0.18; animation-delay: calc(2 * var(--sketch-step) - var(--sketch-loop)); }}
	.a.stabilize .dot {{ left: 66px; top: 44px; }}
	{sketch_clip}
	{sketch_edge}

	/* ── B · Photos: one train-clouds photo cropped eight ways; the stabilized clip itself,
	   all {FRAMES} frames at {CLIP_FPS} fps, the cloud under the point throughout. */
	.b .visual img {{ position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; display: block; filter: saturate(0.75) brightness(0.9); }}
	.b.stabilize img {{ opacity: 0; animation: page var(--clip-loop) step-end calc((var(--i) - {FRAMES}) * {1 / CLIP_FPS}s) infinite; }}
	@keyframes page {{
		0% {{ opacity: 1; }}
		{page_pct}%, 100% {{ opacity: 0; }}
	}}
	.b.stabilize .dot {{ left: 65.3px; top: 24.1px; }}

	/* ── C · Pages: the flip book laid flat, four pages at once, the brand colour steps along them at 4 a second. */
	.c .pages {{ position: absolute; inset: 0; display: grid; grid-template-columns: repeat(2, 62px); gap: 8px; align-content: center; }}
	.c .page {{ position: relative; height: 42px; border: 1.5px solid var(--mark); overflow: hidden; animation: lit 1s step-end calc(var(--i) * -0.25s) infinite; }}
	.c .page .scene {{ opacity: 0.5; }}
	.c.enframe .page {{ animation-name: none; }}
	.c.enframe .page .crop {{ position: absolute; border: 1.5px solid var(--blue); animation: lit-box 1s step-end calc(var(--i) * -0.25s) infinite; }}
	@keyframes lit {{
		0% {{ border-color: var(--amber); }}
		25%, 100% {{ border-color: var(--mark); }}
	}}
	@keyframes lit-box {{
		0% {{ opacity: 1; }}
		25%, 100% {{ opacity: 0.3; }}
	}}
	.c.stabilize .page .dot {{ width: 6px; height: 6px; margin: -3px 0 0 -3px; box-shadow: none; }}

	@media (prefers-reduced-motion: reduce) {{
		.visual *, .visual *::after {{ animation: none !important; }}
		.b.stabilize img:first-child {{ opacity: 1; }}
	}}
	body:has(#still:checked) .visual *, body:has(#still:checked) .visual *::after {{ animation: none !important; }}
	body:has(#still:checked) .b.stabilize img:first-child {{ opacity: 1; }}
</style>
</head>
<body>

<svg width="0" height="0" style="position:absolute" aria-hidden="true">
	<symbol id="scene" viewBox="0 0 132 92" preserveAspectRatio="xMidYMid slice">
		<path d="M0 70 C 20 60, 34 64, 50 56 S 86 52, 104 60 S 124 66, 132 62 V 92 H 0 Z" fill="#3f3f46"/>
		<path d="M0 80 C 30 72, 60 78, 90 74 S 120 70, 132 76 V 92 H 0 Z" fill="#27272a"/>
		<circle cx="66" cy="44" r="7" fill="#71717a"/>
	</symbol>
</svg>

<header class="note">
	<div>
		<h1>Start screens, as a pair · 3 variants</h1>
		<p>enframe tells many stories from one image: the image holds, the framing changes. Stabilize tells one thing's story across many images: the thing holds, the images flip past like the pages of a flip book, {SKETCH_FPS} a second drawn and {CLIP_FPS} a second on photos. Every move is a cut. Taglines and visuals mix freely across rows.</p>
	</div>
	<label><input type="checkbox" id="still"> Still frames (reduced motion)</label>
</header>

<section class="row">
	<h2>A · Sketch <span>· enframe's corner marks on a drawn scene; the crop steps, the photo flips</span></h2>
	<div class="pair">
		<div class="screen enframe a">
			<div class="visual" aria-hidden="true">
				<svg class="scene"><use href="#scene"/></svg>
				<div class="crop flip ghost2"><span class="mark tl"></span><span class="mark tr"></span><span class="mark bl"></span><span class="mark br"></span></div>
				<div class="crop flip ghost"><span class="mark tl"></span><span class="mark tr"></span><span class="mark bl"></span><span class="mark br"></span></div>
				<div class="crop flip live"><span class="mark tl"></span><span class="mark tr"></span><span class="mark bl"></span><span class="mark br"></span></div>
			</div>
			<h1>One photo, <b>many stories</b></h1>
			<p class="lede">Drop an image and frame it more than one way. Each crop is its own story, saved next to the photo.</p>
			<button class="open">Choose an image</button>
		</div>
		<div class="screen stabilize a">
			<div class="visual" aria-hidden="true">
				<span class="edge flip ghost2"></span>
				<span class="edge flip ghost"></span>
				<div class="photo flip"><svg class="scene"><use href="#scene"/></svg></div>
				<span class="dot"></span>
			</div>
			<h1>Many photos, <b>one thing</b></h1>
			<p class="lede">Open a folder of photos. Mark the cloud, the mountain, the moon, whatever should stay put, and every photo lines up around it.</p>
			<button class="open">Open folder</button>
		</div>
	</div>
</section>

<section class="row">
	<h2>B · Photos <span>· one train-clouds photo framed eight ways; the stabilized clip, all {FRAMES} frames at {CLIP_FPS} fps</span></h2>
	<div class="pair">
		<div class="screen enframe b">
			<div class="visual" aria-hidden="true">
				<img src="assets/IMG_4667.jpg" alt="">
				<div class="crop flip ghost2"><span class="mark tl"></span><span class="mark tr"></span><span class="mark bl"></span><span class="mark br"></span></div>
				<div class="crop flip ghost"><span class="mark tl"></span><span class="mark tr"></span><span class="mark bl"></span><span class="mark br"></span></div>
				<div class="crop flip live"><span class="mark tl"></span><span class="mark tr"></span><span class="mark bl"></span><span class="mark br"></span></div>
			</div>
			<h1>Every story <b>in one image</b></h1>
			<p class="lede">Drop an image and frame it more than one way. Each crop is its own story, saved next to the photo.</p>
			<button class="open">Choose an image</button>
		</div>
		<div class="screen stabilize b">
			<div class="visual" aria-hidden="true">
{clip_imgs}
				<span class="dot"></span>
			</div>
			<h1>One thing <b>through every image</b></h1>
			<p class="lede">Open a folder of photos. Mark the cloud, the mountain, the moon, whatever should stay put, and every photo lines up around it.</p>
			<button class="open">Open folder</button>
		</div>
	</div>
</section>

<section class="row">
	<h2>C · Pages <span>· the flip book laid flat: four pages at once, the brand colour steps along them</span></h2>
	<div class="pair">
		<div class="screen enframe c">
			<div class="visual" aria-hidden="true">
				<div class="pages">
					<div class="page" style="--i:0"><svg class="scene"><use href="#scene"/></svg><span class="crop" style="inset:4px 20px 4px 3px"></span></div>
					<div class="page" style="--i:1"><svg class="scene"><use href="#scene"/></svg><span class="crop" style="inset:3px 4px 3px 32px"></span></div>
					<div class="page" style="--i:2"><svg class="scene"><use href="#scene"/></svg><span class="crop" style="inset:10px 12px 12px 10px"></span></div>
					<div class="page" style="--i:3"><svg class="scene"><use href="#scene"/></svg><span class="crop" style="inset:16px 3px 3px 20px"></span></div>
				</div>
			</div>
			<h1>Frame it <b>a dozen ways</b></h1>
			<p class="lede">Drop an image and frame it more than one way. Each crop is its own story, saved next to the photo.</p>
			<button class="open">Choose an image</button>
		</div>
		<div class="screen stabilize c">
			<div class="visual" aria-hidden="true">
				<div class="pages">
					<div class="page" style="--i:0"><svg class="scene" style="translate:0 0"><use href="#scene"/></svg><span class="dot" style="left:31px;top:20.7px"></span></div>
					<div class="page" style="--i:1"><svg class="scene" style="translate:-8px 3px"><use href="#scene"/></svg><span class="dot" style="left:23px;top:23.7px"></span></div>
					<div class="page" style="--i:2"><svg class="scene" style="translate:6px -4px"><use href="#scene"/></svg><span class="dot" style="left:37px;top:16.7px"></span></div>
					<div class="page" style="--i:3"><svg class="scene" style="translate:-3px 6px"><use href="#scene"/></svg><span class="dot" style="left:28px;top:26.7px"></span></div>
				</div>
			</div>
			<h1>Hold it <b>through every frame</b></h1>
			<p class="lede">Open a folder of photos. Mark the cloud, the mountain, the moon, whatever should stay put, and every photo lines up around it.</p>
			<button class="open">Open folder</button>
		</div>
	</div>
</section>

</body>
</html>
"""

OUT.write_text(html)
print(OUT, len(html), 'bytes')
