# Contributing

## Prerequisites

- [rustup](https://rustup.rs). The toolchain is pinned in `rust-toolchain.toml`
  and installs on first use. The supported minimum (`rust-version`) is in the
  workspace `Cargo.toml`.
- [just](https://github.com/casey/just)
- [uv](https://docs.astral.sh/uv/) for the Python bindings and notebooks
- Optional: [cargo-nextest](https://nexte.st), [cargo-machete](https://github.com/bnjbvr/cargo-machete),
  [typos](https://github.com/crate-ci/typos) (`just check` uses all three)
- Optional: pre-commit hooks, via `uvx pre-commit install`

## Before you push

CI gates on the same checks `just check` runs:

| Check | Command |
|---|---|
| Format | `cargo fmt --all --check` |
| Lints | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` |
| Unused deps | `cargo machete` |
| Python | `uvx ruff check .` and `uvx ruff format --check .` |
| Spelling | `typos` |
| Tests | `cargo nextest run --workspace --all-features --locked` plus `cargo test --doc` |
| Rustdoc | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` |
| MSRV | `just msrv` |

`just fix` applies clippy's auto-fixes and then formats.

CI does not build the Python package. After changing `python/` (or the
simulations it wraps), run `just test-py`: it builds `caspers` into
`python/.venv` and runs `python/tests`.

## Dependencies

Every external dependency is declared once in the root `Cargo.toml` under
`[workspace.dependencies]`. Crates reference it with `{ workspace = true }` and
add features locally where needed.

## Commits and pull requests

- Commit messages and PR titles are [conventional commits](https://www.conventionalcommits.org):
  `<type>(<scope>): <subject>`, with types `feat fix refactor docs test chore`
  (plus `perf build ci`). The scope is the crate's package name, or nothing
  for workspace-wide changes.
- PRs are squash-merged, so the PR title becomes the commit on `main`. CI lints it.
- Record user-visible changes under `[Unreleased]` in `docs/CHANGELOG.md`.

## Releases

Python wheels are built and published to PyPI by `.github/workflows/release.yml`
when a `v*` tag is pushed. The Rust crates are not published to crates.io.
