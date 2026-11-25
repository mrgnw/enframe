# /// script
# requires-python = '>=3.14'
# dependencies = ['pyobjc-framework-Quartz==12.1']
# ///
import sys

import Quartz

TITLE = sys.argv[1]
windows = Quartz.CGWindowListCopyWindowInfo(Quartz.kCGWindowListOptionOnScreenOnly, Quartz.kCGNullWindowID)
for window in windows:
	if window.get('kCGWindowName') == TITLE:
		bounds = window['kCGWindowBounds']
		print(window['kCGWindowNumber'], int(bounds['X']), int(bounds['Y']), int(bounds['Width']), int(bounds['Height']))
		break
