# /// script
# requires-python = ">=3.12"
# dependencies = ["pyobjc-framework-Quartz"]
# ///
import subprocess
import sys

import Quartz

windows = Quartz.CGWindowListCopyWindowInfo(Quartz.kCGWindowListOptionOnScreenOnly, Quartz.kCGNullWindowID)
target = next(
	(w for w in windows if w.get('kCGWindowOwnerName', '').lower() == 'enframe'),
	None,
)
if target is None:
	sys.exit('no enframe window on screen')

window_id = target['kCGWindowNumber']
out = sys.argv[1] if len(sys.argv) > 1 else '/tmp/enframe_window.png'
subprocess.run(['screencapture', '-o', f'-l{window_id}', out], check=True)
print(out)
