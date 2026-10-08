"""Copy mockups/ into the main checkout so dmox (which serves **/mockups under ~/dev) can reach a worktree's files."""

import shutil
import sys
from pathlib import Path

SRC = Path(__file__).resolve().parent.parent / 'mockups'
DST = Path('/Users/user/dev/pinframe/mockups')


def main() -> None:
	assert SRC.is_dir(), SRC
	if DST.is_symlink() or DST.is_file():
		DST.unlink()
	shutil.copytree(SRC, DST, dirs_exist_ok=True)
	print(f'{SRC} -> {DST}')


if __name__ == '__main__':
	sys.exit(main())
