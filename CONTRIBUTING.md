# Contributing to 2389 Agent Protocol (Rust)

Thank you for your interest in contributing! This document provides guidelines for contributing to this project.

## Development Setup

### Prerequisites

- Rust 1.75 or later
- MQTT broker (Mosquitto recommended)
- Git

### Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/2389-agent-rust`
3. Create a feature branch: `git checkout -b feature/my-feature`
4. Make your changes
5. Run tests: `cargo test`
6. Submit a pull request

## Development Workflow

### Code Quality

Before submitting, ensure your code passes all quality gates:

```bash
# Format code
cargo fmt

# Lint
cargo clippy --all-targets -- -D warnings

# Run tests
cargo test
```

### Testing

- Write tests for all new functionality
- Maintain ≥80% test coverage
- Run `cargo test` before committing
- Integration tests require Docker for MQTT broker

### Commit Messages

Use conventional commit format:
- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation changes
- `test:` - Test additions/changes
- `chore:` - Maintenance tasks
- `refactor:` - Code refactoring

Example: `feat: add support for custom tool parameters`

## Code Style

- Follow existing code patterns
- Use `cargo fmt` for formatting
- Address all `cargo clippy` warnings
- Document public APIs with rustdoc comments
- Include examples in documentation

## Pull Request Process

1. Update documentation for any new features
2. Add tests for bug fixes and new features
3. Ensure all tests pass locally
4. Update README.md if needed
5. Request review from maintainers

## Testing Standards

- Unit tests for all modules
- Integration tests for MQTT communication
- Property-based tests for protocol edge cases
- All tests must pass before merging

## Questions?

- Open an issue for bugs or feature requests
- Use discussions for questions
- Check existing issues before creating new ones

## License

By contributing, you agree that your contributions will be licensed under the same MIT OR Apache-2.0 dual license as the project.
