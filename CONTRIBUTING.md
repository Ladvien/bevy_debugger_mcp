# Contributing to Bevy Debugger MCP

Thank you for your interest in contributing to Bevy Debugger MCP! This document provides guidelines and information for contributors.

## 🚀 Quick Start

1. **Fork** the repository
2. **Clone** your fork: `git clone https://github.com/yourusername/bevy_debugger_mcp.git`
3. **Create** a feature branch: `git checkout -b feature-name`
4. **Make** your changes
5. **Test** your changes: `cargo test`
6. **Submit** a pull request

## 📋 Development Setup

### Prerequisites

- Rust 1.70 or later
- Cargo
- Git

### Local Development

```bash
# Clone the repository
git clone https://github.com/ladvien/bevy_debugger_mcp.git
cd bevy_debugger_mcp

# Install dependencies and build
cargo build

# Run tests
cargo test

# Check formatting and linting
cargo fmt --check
cargo clippy -- -D warnings

# Run with debug output
RUST_LOG=debug cargo run
```

## 🔍 Code Style

We follow standard Rust conventions:

- **Formatting**: Use `cargo fmt` (rustfmt)
- **Linting**: Use `cargo clippy` 
- **Documentation**: Document public APIs with `///` comments
- **Testing**: Write tests for new functionality

### Code Organization

```
src/
├── main.rs              # Entry point and CLI
├── mcp_server.rs        # MCP protocol implementation  
├── brp_client.rs        # Bevy Remote Protocol client
├── tools/               # Debugging tools
│   ├── observe.rs       # Entity observation
│   ├── experiment.rs    # State experimentation
│   └── ...
├── error.rs             # Error types and handling
└── config.rs            # Configuration management
```

## 🧪 Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test module
cargo test test_observe

# Run with output
cargo test -- --nocapture

# Run integration tests
cargo test --test integration_test
```

### Writing Tests

- **Unit tests**: Place in the same file as the code being tested
- **Integration tests**: Place in the `tests/` directory
- **Documentation tests**: Include examples in doc comments

Example:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_observe_entities() {
        // Test implementation
    }
}
```

## 📝 Commit Guidelines

We follow conventional commit format:

```
type(scope): brief description

Longer description if necessary

Fixes #issue-number
```

### Commit Types

- **feat**: New feature
- **fix**: Bug fix
- **docs**: Documentation changes
- **style**: Code style changes (formatting, etc.)
- **refactor**: Code refactoring
- **test**: Adding or modifying tests
- **chore**: Maintenance tasks

### Examples

```bash
feat(tools): add entity filtering to observe tool
fix(brp): handle connection timeout gracefully
docs(readme): update installation instructions
```

## 🐛 Bug Reports

When reporting bugs, please include:

1. **Environment**: OS, Rust version, Bevy version
2. **Steps to reproduce** the issue
3. **Expected behavior**
4. **Actual behavior**
5. **Error messages** or logs
6. **Minimal example** if possible

Use the bug report template when creating issues.

## 💡 Feature Requests

For feature requests, please:

1. **Search existing issues** to avoid duplicates
2. **Describe the use case** and motivation
3. **Propose a solution** if you have one
4. **Consider backwards compatibility**

## 🔧 Pull Request Process

1. **Create an issue** first for significant changes
2. **Fork and branch** from `main`
3. **Write tests** for new functionality
4. **Update documentation** as needed
5. **Ensure CI passes** (tests, formatting, linting)
6. **Write descriptive commit messages**
7. **Request review** from maintainers

### PR Checklist

- [ ] Tests pass locally (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] No linting errors (`cargo clippy`)
- [ ] Documentation updated if needed
- [ ] Changelog entry added (if applicable)
- [ ] Issue linked in PR description

## 🏗️ Architecture Guidelines

### Adding New Tools

When adding debugging tools:

1. Create a new file in `src/tools/`
2. Implement the `ToolHandler` trait
3. Add to the tool registry in `mcp_server.rs`
4. Write comprehensive tests
5. Document the tool's purpose and usage

### Error Handling

- Use the custom `Error` type defined in `error.rs`
- Provide helpful error messages
- Add context with `ErrorContext` for debugging
- Handle errors gracefully without panicking

### Async Code

- Use `tokio` for async runtime
- Prefer `async/await` over futures combinators
- Use appropriate synchronization primitives (`RwLock`, `Mutex`)
- Handle cancellation correctly

## 📚 Documentation

### Code Documentation

- Document all public APIs
- Include examples in doc comments
- Explain complex algorithms or design decisions
- Keep documentation up to date with code changes

### User Documentation

- Update README.md for user-facing changes
- Add examples for new features
- Update the usage guide as needed
- Consider adding screenshots or videos

## 🚦 CI/CD

Our CI pipeline runs:

- **Tests**: Unit and integration tests
- **Formatting**: `cargo fmt --check`
- **Linting**: `cargo clippy`
- **Security**: `cargo audit`
- **Documentation**: `cargo doc`

Make sure all checks pass before submitting a PR.

## 🤝 Code of Conduct

We follow the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Please be respectful and inclusive in all interactions.

## 📞 Getting Help

- **Issues**: For bugs and feature requests
- **Discussions**: For questions and ideas
- **Discord**: Join the Bevy community Discord

## 🎉 Recognition

Contributors are automatically added to the contributors list in our README. Significant contributors may be invited to become maintainers.

Thank you for contributing to Bevy Debugger MCP! 🚀