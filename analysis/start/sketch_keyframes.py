# /// script
# requires-python = ">=3.12"
# ///
# Prints the Stabilize start visual's keyframes: a 96 x 72 photo wandering inside 132 x 92 so the
# moon at (66, 44) stays inside it. Pasted into src/lib/StartVisual.svelte.
import random

rng = random.Random(29)
left, top = 14.0, 8.0
out = []
for _ in range(30):
	left = min(36, max(0, left + rng.uniform(-7, 7)))
	top = min(20, max(0, top + rng.uniform(-5, 5)))
	out.append((round(top, 1), round(36 - left, 1), round(20 - top, 1), round(left, 1)))


def pct(i: int) -> str:
	return f'{100 * i / len(out):.3f}'.rstrip('0').rstrip('.')


print('\t@keyframes photo-clip {')
for i, (t, r, b, l) in enumerate(out):
	print(f'\t\t{pct(i)}% {{ clip-path: inset({t}px {r}px {b}px {l}px); }}')
print('\t}\n\t@keyframes photo-edge {')
for i, (t, r, b, l) in enumerate(out):
	print(f'\t\t{pct(i)}% {{ inset: {t}px {r}px {b}px {l}px; }}')
print('\t}')
