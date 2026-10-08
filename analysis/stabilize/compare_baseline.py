"""Compare a tracking benchmark report with the committed baseline, ignoring run time.

uv run analysis/stabilize/tracking_benchmark.py
uv run analysis/stabilize/compare_baseline.py out/tracking_benchmark.json analysis/stabilize/tracking_baseline.json
"""

import json
import sys
from pathlib import Path

report, baseline = (json.loads(Path(path).read_text()) for path in sys.argv[1:3])
for data in (report, baseline):
	data.pop('seconds', None)
if report != baseline:
	sys.exit('DIFFERS from baseline')
print('identical to baseline')
