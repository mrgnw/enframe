"""Encode the Girona frames with each candidate codec: time, CPU, size and fidelity.

FFMPEG=/opt/homebrew/bin/ffmpeg uv run analysis/codecs.py
Renders the black-padded and transparent (--pad transparent) frame sets once into out/codecs/.
"""

import os
import resource
import shutil
import subprocess
import time
from pathlib import Path

import numpy as np
from PIL import Image, ImageSequence

ROOT = Path(__file__).parent.parent
WORK = ROOT / 'out' / 'codecs'
FPS = 4
SAMPLE_EVERY = 4
FFMPEG = os.environ.get('FFMPEG', 'ffmpeg')


def hevc(q: int, alpha: bool = False):
	label = f'HEVC{"+alpha" if alpha else ""} VideoToolbox HW q {q}'
	pixels = ['-alpha_quality', '0.75', '-pix_fmt', 'bgra'] if alpha else ['-pix_fmt', 'yuv420p']
	return (
		label,
		'.mov' if alpha else '.mp4',
		['-c:v', 'hevc_videotoolbox', '-q:v', str(q), '-tag:v', 'hvc1', *pixels],
	)


def svt(preset: int, crf: int):
	args = ['-c:v', 'libsvtav1', '-preset', str(preset), '-crf', str(crf), '-pix_fmt', 'yuv420p']
	return f'AV1 SVT-AV1 preset {preset} crf {crf}', '.mp4', args


def avif(speed: int, quality: int):
	return f'AVIF animated, aom speed {speed} q {quality}', '.avif', {'speed': speed, 'quality': quality}


OPAQUE = [
	('H.264 x264 crf 18 (current .mp4)', '.mp4', ['-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '18']),
	*[hevc(q) for q in (65, 75, 80, 85)],
	*[svt(10, crf) for crf in (32, 27, 23)],
	svt(8, 27),
	avif(10, 75),
	avif(10, 85),
]
ALPHA = [
	(
		'ProRes 4444 (current .mov)',
		'.mov',
		['-c:v', 'prores_ks', '-profile:v', '4444', '-pix_fmt', 'yuva444p10le'],
	),
	(
		'VP9 alpha crf 30 (current .webm)',
		'.webm',
		['-c:v', 'libvpx-vp9', '-pix_fmt', 'yuva420p', '-crf', '30', '-b:v', '0'],
	),
	*[hevc(q, alpha=True) for q in (75, 80, 85)],
	avif(10, 75),
	avif(10, 85),
]


def render_frames(name: str, pad: str) -> Path:
	frames_dir = WORK / name
	if not any(frames_dir.glob('*.png')):
		shutil.rmtree(frames_dir, ignore_errors=True)
		env = os.environ | {'PAD': pad}
		cmd = [
			'bash',
			'analysis/girona_all.sh',
			str(WORK / f'{name}-render.mov'),
			'--frames',
			str(frames_dir),
		]
		subprocess.run(cmd, cwd=ROOT, env=env, check=True, stdout=subprocess.DEVNULL)
	return frames_dir


def load(frames_dir: Path, alpha: bool) -> list[np.ndarray]:
	mode = 'RGBA' if alpha else 'RGB'
	return [np.asarray(Image.open(p).convert(mode)) for p in sorted(frames_dir.glob('*.png'))]


def children_cpu() -> float:
	usage = resource.getrusage(resource.RUSAGE_CHILDREN)
	return usage.ru_utime + usage.ru_stime


def encode_ffmpeg(frames, alpha: bool, args: list[str], out: Path) -> tuple[float, float]:
	h, w = frames[0].shape[:2]
	cmd = [
		FFMPEG, '-y', '-loglevel', 'error',
		'-f', 'rawvideo', '-pix_fmt', 'rgba' if alpha else 'rgb24', '-s', f'{w}x{h}', '-r', str(FPS), '-i', '-',
		*args, str(out),
	]  # fmt: skip
	cpu, start = children_cpu(), time.perf_counter()
	subprocess.run(cmd, input=b''.join(f.tobytes() for f in frames), check=True)
	return time.perf_counter() - start, children_cpu() - cpu


def encode_avif(frames, options: dict, out: Path) -> tuple[float, float]:
	images = [Image.fromarray(f) for f in frames]
	cpu, start = time.process_time(), time.perf_counter()
	images[0].save(out, 'AVIF', save_all=True, append_images=images[1:], duration=1000 // FPS, **options)
	return time.perf_counter() - start, time.process_time() - cpu


def decode(out: Path, alpha: bool, shape) -> list[np.ndarray]:
	if out.suffix == '.avif':
		with Image.open(out) as im:
			return [
				np.asarray(frame.convert('RGBA' if alpha else 'RGB')) for frame in ImageSequence.Iterator(im)
			]
	h, w = shape[:2]
	channels = 4 if alpha else 3
	# libvpx decodes VP9 alpha; ffmpeg's native vp9 decoder drops it.
	codec = ['-c:v', 'libvpx-vp9'] if out.suffix == '.webm' else []
	cmd = [
		FFMPEG, '-loglevel', 'error', *codec, '-i', str(out),
		'-f', 'rawvideo', '-pix_fmt', 'rgba' if alpha else 'rgb24', '-',
	]  # fmt: skip
	raw = subprocess.run(cmd, capture_output=True, check=True).stdout
	return list(np.frombuffer(raw, np.uint8).reshape(-1, h, w, channels))


def psnr(a: np.ndarray, b: np.ndarray) -> float:
	mse = np.mean((a.astype(np.float64) - b.astype(np.float64)) ** 2)
	return float('inf') if mse == 0 else 10 * np.log10(255**2 / mse)


def measure(label: str, frames, alpha: bool, suffix: str, spec, out: Path) -> str:
	if isinstance(spec, dict):
		wall, cpu = encode_avif(frames, spec, out)
	else:
		wall, cpu = encode_ffmpeg(frames, alpha, spec, out)
	decoded = decode(out, alpha, frames[0].shape)
	assert len(decoded) == len(frames), f'{label}: decoded {len(decoded)} of {len(frames)} frames'
	pairs = list(zip(decoded[::SAMPLE_EVERY], frames[::SAMPLE_EVERY]))
	rgb = np.mean([psnr(d[..., :3], f[..., :3]) for d, f in pairs])
	row = f'| {label} | {wall:.2f} | {cpu:.1f} | {out.stat().st_size / 1e6:.1f} | {rgb:.1f}'
	if alpha:
		# A dropped alpha channel scores a few dB here, so no separate check is needed.
		row += f' | {np.mean([psnr(d[..., 3], f[..., 3]) for d, f in pairs]):.1f} |'
	else:
		row += ' |'
	return row


def available_encoders() -> set[str]:
	listing = subprocess.run(
		[FFMPEG, '-hide_banner', '-encoders'], capture_output=True, text=True, check=True
	).stdout
	return {line.split()[1] for line in listing.splitlines() if line.startswith(' V')}


def table(title: str, frames_dir: Path, alpha: bool, configs):
	encoders = available_encoders()
	frames = load(frames_dir, alpha)
	h, w = frames[0].shape[:2]
	print(f'\n{title}: {len(frames)} frames at {w}x{h}\n')
	print('| encoding | wall s | CPU s | MB | PSNR RGB dB |' + (' PSNR alpha dB |' if alpha else ''))
	print('|-|-|-|-|-|' + ('-|' if alpha else ''))
	for i, (label, suffix, spec) in enumerate(configs):
		if isinstance(spec, list) and spec[spec.index('-c:v') + 1] not in encoders:
			print(f'| {label} | not in this ffmpeg build |')
			continue
		print(
			measure(label, frames, alpha, suffix, spec, WORK / f'{frames_dir.name}-{i}{suffix}'), flush=True
		)


if __name__ == '__main__':
	WORK.mkdir(parents=True, exist_ok=True)
	# The default crop (no --pad) shrinks to 392x368 on Girona, too small to tell encoders apart.
	table('Opaque, --pad black', render_frames('black', 'black'), False, OPAQUE)
	table('Transparent, --pad transparent', render_frames('alpha', 'transparent'), True, ALPHA)
