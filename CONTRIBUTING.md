# Contributing to CreoBench

Thanks for your interest in contributing! This guide covers everything you
need to get up and running.

## Prerequisites

- **Rust** (stable) — install via [rustup](https://rustup.rs/)
- **gh** (GitHub CLI) — optional, but useful for issue and PR workflows

## Code of Conduct

This project and everyone participating in it is governed by our [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code. Please report unacceptable behavior to [yannik.lubas@uni-wuerzburg.de](mailto:yannik.lubas@uni-wuerzburg.de).

## Building

```sh
cargo build
```

## Testing

```sh
cargo test --locked --all-features --all-targets
```

Run a single test file:

```sh
cargo test --locked --all-features --test <test_name>
```

Run doc tests:

```sh
cargo test --locked --all-features --doc
```

## Formatting

We use `rustfmt` with the default stable configuration. CI will fail if
code is not formatted.

```sh
cargo fmt --check   # check only
cargo fmt            # auto-format
```

## Linting

We use `clippy` with `-Dwarnings` (all warnings are errors). Run it
before submitting:

```sh
cargo clippy --all-features --all-targets -- -Dwarnings
```

## Documentation

If your change affects user-facing behaviour, update the relevant
documentation in `docs/` as part of the same PR.

If your change introduces a new concept or command, consider adding a short
entry to the appropriate doc page (see [`docs/README.md`](docs/README.md) for the full
index).

## Commit conventions

This project follows [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>[optional scope]: <description>
```

### Allowed types

| Type       | When to use                                 |
| ---------- | ------------------------------------------- |
| `feat`     | A new feature or user-facing capability     |
| `fix`      | A bug fix                                   |
| `refactor` | Code restructuring with no behaviour change |
| `chore`    | Tooling, dependencies, CI, formatting       |
| `docs`     | Documentation changes                       |
| `test`     | Adding or correcting tests                  |

### Rules

- Use the **imperative mood** in the description (`add feature`, not
  `added feature`).
- Keep the subject line under **72 characters**.
- Reference issue numbers at the end of the subject line when applicable:
  `feat: add CSV export (#42)`.
- A `!` after the type/scope signals a breaking change:
  `feat!: remove deprecated config option`.

Examples:

```
feat: add transaction-level CSV output (#142)
feat(cli)!: remove `--bogus` CLI flag
fix(virtual_user): prevent cookie jar race on concurrent VUs
refactor: extract dispatcher loop into library (#136)
chore(deps): bump tokio from 1.52.3 to 1.53.1
docs: update CONTRIBUTING.md
test: add integration test for warmup phase
```

### Finding work

1. Open [GitHub Issues](https://github.com/DescartesResearch/CreoBench/issues).
2. Filter by `good first issue` to find good first issues.

### Reporting bugs

Use the **Bug Report** issue template. Include reproduction steps, expected
vs. actual behaviour, and your project version.

### Requesting features

Use the **Feature Request** issue template. Describe the problem you are
trying to solve, not just the solution you envision.

## Pull request process

1. Fork the repository and create a feature branch from `main`.
2. Make your changes, following the commit conventions above.
3. Ensure all checks pass locally:
   ```sh
   cargo fmt --check
   cargo clippy --all-features --all-targets -- -Dwarnings
   cargo test --all-features --all-targets
   ```
4. Open a PR against `main`. Fill out the PR template.
5. A maintainer will review and may request changes before merging.

## Licence

By contributing, you agree that your contributions will be licensed under
the **GNU Affero General Public License v3.0** (see [LICENSE](LICENSE)).
