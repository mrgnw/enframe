# Exports every photo in ~/dev/enframe/images as AVIF through a running enframe-serve, one folder per
# setting, and prints size and time. Start serve first:
# src-tauri/target/release/enframe-serve --port 5987 --read-root ~/dev/enframe/images --write-root ~/dev/_outbox/enframe-avif-compare

# /// script
# requires-python = '>=3.14'
# dependencies = ['httpx']
# ///
import statistics
import sys
import time
from pathlib import Path

import httpx

SERVE = 'http://127.0.0.1:5987'
SOURCES = Path.home() / 'dev/enframe/images'
OUT = Path.home() / 'dev/_outbox/enframe-avif-compare'
RUNS = [('q92', 92, 9), ('q87', 87, 9), ('today-q92-speed6', 92, 6)]


def export(client: httpx.Client, source: Path, output: Path, quality: int, speed: int) -> float:
	body = {
		'sourcePath': str(source),
		'outputPath': str(output),
		'x': 0.0,
		'y': 0.0,
		'width': 1.0,
		'height': 1.0,
		'quality': quality,
		'speed': speed,
		'format': 'avif',
	}
	started = time.perf_counter()
	response = client.post(f'{SERVE}/api/crop_and_export', json=body)
	elapsed = time.perf_counter() - started
	response.raise_for_status()
	return elapsed


def main() -> None:
	sources = sorted(p for p in SOURCES.iterdir() if p.suffix.lower() in {'.jxl', '.jpg', '.jpeg'})
	wanted = sys.argv[1:] or ['q92', 'q87']
	runs = [run for run in RUNS if run[0] in wanted]
	with httpx.Client(timeout=120) as client:
		for folder, quality, speed in runs:
			(OUT / folder).mkdir(parents=True, exist_ok=True)
			times = []
			for source in sources:
				times.append(export(client, source, OUT / folder / f'{source.stem}.avif', quality, speed))
			size = sum(p.stat().st_size for p in (OUT / folder).glob('*.avif'))
			print(
				f'{folder}: {len(sources)} files, {size / 1024:.0f} KB total, '
				f'{size / 1024 / len(sources):.0f} KB each, median {statistics.median(times) * 1000:.0f} ms'
			)


if __name__ == '__main__':
	main()
