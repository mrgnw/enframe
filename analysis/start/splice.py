# /// script
# requires-python = ">=3.12"
# ///
# Replaces the @@KEYFRAMES@@ marker in StartVisual.svelte with the generated keyframes.
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[2]
component = root / 'src/lib/StartVisual.svelte'
frames = subprocess.run(['uv', 'run', '-q', root / 'analysis/start/sketch_keyframes.py'], capture_output=True, text=True, check=True).stdout
text = component.read_text()
assert '@@KEYFRAMES@@' in text
component.write_text(text.replace('@@KEYFRAMES@@', frames.rstrip('\n')))
print('spliced', frames.count('clip-path'), 'steps')
