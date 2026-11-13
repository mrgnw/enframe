# Contributing to Enframe

Small fixes, tests, documentation, and bug reports are welcome. Discuss larger changes in
an issue first. [Good first issues](https://github.com/mrgnw/enframe/issues?q=is%3Aissue%20is%3Aopen%20label%3A%22good%20first%20issue%22)
include a starting file and an expected result.

## Setup

Follow the [macOS prerequisites and quick start](README.md#quick-start-macos).
The [browser framer](README.md#try-just-the-framer-in-a-browser) is enough for many interface changes.
The native app uses port 5910 and its backend uses 5911; the browser example uses 5918.

## Checks

From the repository root:

```sh
bun run check
bun run test
bun run lint
```

For Rust library changes, build the frontend and stage the sidecar first:

```sh
bun run sveltekit:build
bun run sidecar
cargo test --release --manifest-path src-tauri/Cargo.toml --lib
```

Use release mode for the library suite: it includes a face-detection timing check.

The Stabilize walkthroughs also require the external photo series linked at `samples/`.
Those photos are not included in this public repository. See the
[Stabilize guide](docs/stabilize/README.md) before working on tracking or clip export.

## Branch and pull request flow

Fork the repository on GitHub, then clone your fork. Set your GitHub username once:

```sh
GITHUB_USER=
```

```sh
git clone "https://github.com/$GITHUB_USER/enframe.git"
cd enframe
git remote add upstream https://github.com/mrgnw/enframe.git
git fetch upstream
git switch -c improve-enframe upstream/main
```

Use a branch for each concern. Use tabs for indentation. Commit each verified change,
push your branch to your fork, and open a pull request against `mrgnw/enframe`'s `main`.
Explain the user-visible change, link the issue, and list the checks you ran.
Include a screenshot for interface changes, using a photo without people or personal metadata.
Maintainers rebase merge pull requests; keep unrelated changes in separate pull requests.

Use the bug or feature issue form for reports. Include reproducible steps and the app mode
(desktop, serve, or browser). Please keep file paths, photos, and logs free of personal data.

Enframe is [MIT licensed](LICENSE). Keep third-party attribution and dependency license notices intact.
