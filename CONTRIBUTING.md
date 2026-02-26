# Contributing to browsercli

Thank you for your interest in contributing! This document provides guidelines and instructions for contributing.

## Development Setup

### Prerequisites

- Rust 1.75 or later
- A Chromium-based browser (Chrome, Chromium, or Edge)
- macOS, Linux, or Windows

### Build

```bash
git clone https://github.com/justinhuangcode/browsercli.git
cd browsercli
cargo build
```

### Run Tests

```bash
# Unit tests + integration tests
cargo test

# With output
cargo test -- --nocapture
```

### Plugin Tests

Plugin unit tests are included in the main test suite. To run only plugin-specific tests:

```bash
cargo test plugins
```

For plugin development guidelines, see [PLUGINS.md](PLUGINS.md).

### Lint

```bash
# Format check
cargo fmt --all -- --check

# Clippy
cargo clippy --all-targets -- -D warnings
```

## Making Changes

1. Fork the repository and create a branch from `main`.
2. Make your changes with clear, focused commits.
3. Add tests for new functionality.
4. Ensure `cargo test`, `cargo fmt --check`, and `cargo clippy` pass.
5. Open a pull request with a clear description of the change.

## Code Style

- Follow standard Rust formatting (`cargo fmt`).
- No clippy warnings (`cargo clippy -- -D warnings`).
- Prefer explicit error handling over `.unwrap()` in non-test code.
- Keep functions focused and reasonably sized.

## Reporting Issues

- Use GitHub Issues for bug reports and feature requests.
- Include reproduction steps, expected behavior, and actual behavior.
- For security vulnerabilities, please email privately instead of opening a public issue.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
