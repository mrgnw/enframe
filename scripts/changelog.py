#!/usr/bin/env python3
"""Fold the change notes in changelog.d/ into CHANGELOG.md under one date, at release time.

	uv run scripts/changelog.py            # draft, edit in $EDITOR, write, delete the notes
	uv run scripts/changelog.py --print    # show the draft, change nothing
	uv run scripts/changelog.py --yes      # write the draft as is
	uv run scripts/changelog.py --date 2026-10-09

Each pull request adds one note, changelog.d/<branch>.md: bullets under `### Added`, `### Changed`,
`### Fixed` or `### Removed`. The draft is those notes merged by section, plus the date's existing entry
if CHANGELOG.md already has one. Edit or replace it freely; an empty draft aborts with nothing changed.
"""

import argparse
import datetime
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NOTES = ROOT / 'changelog.d'
CHANGELOG = ROOT / 'CHANGELOG.md'
ORDER = ['Added', 'Changed', 'Fixed', 'Removed']

Sections = dict[str, list[str]]


def parse_sections(text: str, source: str) -> Sections:
	sections: Sections = {}
	current = None
	for line in text.splitlines():
		if line.startswith('### '):
			current = sections.setdefault(line[4:].strip(), [])
		elif not line.strip():
			continue
		elif current is None:
			raise SystemExit(
				f'{source}: "{line}" is not under a ### heading (Added, Changed, Fixed, Removed)'
			)
		else:
			current.append(line)
	return sections


def merge(*all_sections: Sections) -> Sections:
	merged: Sections = {}
	for sections in all_sections:
		for name, lines in sections.items():
			merged.setdefault(name, []).extend(lines)
	return {name: lines for name, lines in merged.items() if lines}


def render(date: str, sections: Sections) -> str:
	names = [n for n in ORDER if n in sections] + [n for n in sections if n not in ORDER]
	parts = [f'## {date}']
	for name in names:
		parts.append(f'### {name}\n\n' + '\n'.join(sections[name]))
	return '\n\n'.join(parts) + '\n'


def split_changelog(text: str, date: str) -> tuple[str, str, str]:
	"""The header before the first entry, `date`'s entry if it is the newest, and the older entries."""
	at = text.find('\n## ')
	if at < 0:
		return text.rstrip('\n') + '\n', '', ''
	head, rest = text[:at].rstrip('\n') + '\n', text[at + 1 :]
	if not rest.startswith(f'## {date}\n'):
		return head, '', rest
	end = rest.find('\n## ', 1)
	return (head, rest, '') if end < 0 else (head, rest[: end + 1], rest[end + 1 :])


def draft(date: str, notes: list[Path], changelog: str) -> tuple[str, str]:
	"""The proposed entry for `date`, and the changelog with `{entry}` where it goes."""
	head, same_day, older = split_changelog(changelog, date)
	existing = parse_sections(same_day.split('\n', 1)[1] if same_day else '', f'CHANGELOG.md {date}')
	sections = merge(existing, *(parse_sections(n.read_text(), n.name) for n in notes))
	template = head + '\n{entry}' + ('\n' + older if older else '')
	return (render(date, sections) if sections else ''), template


def edit(text: str) -> str:
	editor = os.environ.get('VISUAL') or os.environ.get('EDITOR') or 'vi'
	with tempfile.NamedTemporaryFile('w+', suffix='.md', delete=False) as f:
		f.write(text)
		path = Path(f.name)
	try:
		subprocess.run([*editor.split(), str(path)], check=True)
		return path.read_text()
	finally:
		path.unlink()


def main() -> None:
	parser = argparse.ArgumentParser(description='Fold changelog.d/ into CHANGELOG.md.')
	parser.add_argument('--date', default=datetime.date.today().isoformat())
	parser.add_argument('--print', action='store_true', help='show the draft, change nothing')
	parser.add_argument('--yes', action='store_true', help='write the draft without opening an editor')
	args = parser.parse_args()
	datetime.date.fromisoformat(args.date)
	notes = sorted(p for p in NOTES.glob('*.md') if p.name != 'README.md')
	if not notes:
		sys.exit('changelog.d/ has no notes; nothing to release')
	entry, template = draft(args.date, notes, CHANGELOG.read_text())
	if args.print:
		print(entry, end='')
		return
	if not args.yes:
		entry = edit(entry)
	if not entry.strip():
		sys.exit('empty draft: CHANGELOG.md and changelog.d/ left as they were')
	CHANGELOG.write_text(template.replace('{entry}', entry.strip('\n') + '\n', 1))
	for note in notes:
		note.unlink()
	print(f'CHANGELOG.md: {args.date} written from {len(notes)} notes; the notes are deleted')


if __name__ == '__main__':
	main()
