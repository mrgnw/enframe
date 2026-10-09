"""uv run scripts/test_changelog.py: the release fold on a sample changelog."""

import tempfile
from pathlib import Path

from changelog import draft, parse_sections

HEAD = '# Changelog\n\nIntro.\n'
OLD = '## 2026-10-01\n\n### Added\n\n- Old thing.\n'


def notes(dir: Path, **files: str) -> list[Path]:
	paths = []
	for name, text in files.items():
		path = dir / f'{name}.md'
		path.write_text(text)
		paths.append(path)
	return sorted(paths)


def fill(entry: str, template: str) -> str:
	return template.replace('{entry}', entry, 1)


def main() -> None:
	with tempfile.TemporaryDirectory() as tmp:
		dir = Path(tmp)
		two = notes(
			dir,
			a='### Fixed\n\n- A fix.\n\n### Added\n\n- A feature,\n  over two lines.\n',
			b='### Added\n\n- B feature.\n',
		)

		entry, template = draft('2026-10-09', two, HEAD + '\n' + OLD)
		assert entry == (
			'## 2026-10-09\n\n### Added\n\n- A feature,\n  over two lines.\n- B feature.\n\n### Fixed\n\n- A fix.\n'
		), entry
		assert fill(entry, template) == HEAD + '\n' + entry + '\n' + OLD

		same_day = HEAD + '\n## 2026-10-09\n\n### Added\n\n- Earlier today.\n\n' + OLD
		entry, template = draft('2026-10-09', two[1:], same_day)
		assert '- Earlier today.\n- B feature.' in entry, entry
		assert fill(entry, template).count('## 2026-10-09') == 1
		assert fill(entry, template).endswith(OLD)

		entry, template = draft('2026-10-09', two[1:], HEAD)
		assert fill(entry, template) == HEAD + '\n' + entry

		try:
			parse_sections('- no heading\n', 'bad.md')
			raise AssertionError('a bullet outside a section is refused')
		except SystemExit as e:
			assert 'bad.md' in str(e)
	print('ok')


if __name__ == '__main__':
	main()
