"""Copy mockups/stabilize/ into the main checkout so dmox (which serves **/mockups under ~/dev) can reach a worktree's files."""

import os
import shutil
import sys
from pathlib import Path

SRC = Path(__file__).resolve().parent.parent.parent / 'mockups' / 'stabilize'
DST = Path(os.environ.get('ENFRAME_MAIN_CHECKOUT') or sys.exit('set ENFRAME_MAIN_CHECKOUT')) / 'mockups' / 'stabilize'


def main() -> None:
	assert SRC.is_dir(), SRC
	if DST.is_symlink() or DST.is_file():
		DST.unlink()
	shutil.copytree(SRC, DST, dirs_exist_ok=True)
	print(f'{SRC} -> {DST}')


if __name__ == '__main__':
	sys.exit(main())
