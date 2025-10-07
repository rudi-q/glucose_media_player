# Contributing to glucose

Thank you for your interest in contributing to glucose! We welcome contributions from the community.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Submitting Changes](#submitting-changes)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Documentation](#documentation)
- [Community](#community)

## Code of Conduct

By participating in this project, you agree to maintain a respectful and inclusive environment for all contributors.

## Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/glucose_media_player.git
   cd glucose_media_player
   ```
3. **Add upstream remote**:
   ```bash
   git remote add upstream https://github.com/rudi-q/glucose_media_player.git
   ```

## Development Setup

### Prerequisites

- **Node.js** 20 or higher
- **pnpm** 9 or higher
- **Rust** (latest stable)
- Platform-specific dependencies:
  - **Linux**: `libwebkit2gtk-4.1-dev`, `build-essential`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`
  - **Windows**: No additional dependencies
  - **macOS**: Xcode Command Line Tools

### Installation

1. Install pnpm globally (if not already installed):
   ```bash
   npm install -g pnpm
   ```

2. Install project dependencies:
   ```bash
   pnpm install
   ```

3. Start development server:
   ```bash
   pnpm tauri dev
   ```

### Available Scripts

- `pnpm dev` - Start Vite dev server
- `pnpm build` - Build frontend for production
- `pnpm check` - Run TypeScript/Svelte type checking
- `pnpm tauri dev` - Start Tauri development mode
- `pnpm tauri build` - Build production application
- `pnpm tauri:check` - Run Rust cargo check

## Making Changes

### Branch Naming

Use descriptive branch names with prefixes:
- `feature/` - New features (e.g., `feature/playlist-support`)
- `fix/` - Bug fixes (e.g., `fix/subtitle-timing`)
- `refactor/` - Code refactoring (e.g., `refactor/video-player`)
- `docs/` - Documentation updates (e.g., `docs/keyboard-shortcuts`)
- `chore/` - Maintenance tasks (e.g., `chore/update-dependencies`)

### Commit Messages

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types:**
- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation changes
- `style` - Code style changes (formatting, missing semicolons, etc.)
- `refactor` - Code refactoring
- `perf` - Performance improvements
- `test` - Adding or updating tests
- `chore` - Maintenance tasks
- `ci` - CI/CD changes

**Examples:**
```
feat(subtitles): add support for SRT format

- Implement SRT to WebVTT conversion
- Add auto-detection of subtitle files
- Support manual subtitle file selection

Closes #42
```

```
fix(video): resolve memory leak in thumbnail generation

The canvas was not being properly cleaned up after thumbnail
generation, causing memory to accumulate over time.

Fixes #123
```

### Keep Changes Focused

- One feature or fix per pull request
- Keep pull requests small and reviewable
- Break large changes into multiple PRs when possible

## Submitting Changes

### Before Submitting

1. **Sync with upstream**:
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Run checks**:
   ```bash
   pnpm check
   pnpm tauri:check
   pnpm build
   ```

3. **Test your changes** thoroughly on your platform
4. **Update documentation** if needed

### Creating a Pull Request

1. Push your branch to your fork:
   ```bash
   git push origin feature/your-feature-name
   ```

2. Open a pull request on GitHub

3. Fill out the PR template with:
   - Clear description of changes
   - Related issue numbers
   - Testing steps
   - Screenshots/videos for UI changes

4. Wait for CI checks to pass

5. Address review feedback promptly

### PR Review Process

- All PRs require at least one approval
- CI checks must pass
- Code should follow project standards
- Changes should be tested on at least one platform

## Coding Standards

### TypeScript/Svelte

- Use TypeScript for type safety
- Follow existing code style
- Use Svelte 5 runes syntax (`$state`, `$derived`, etc.)
- Prefer functional components
- Use meaningful variable and function names
- Add JSDoc comments for complex functions

### Rust

- Follow Rust style guidelines (use `cargo fmt`)
- Run `cargo clippy` and address warnings
- Write idiomatic Rust code
- Add documentation comments for public APIs
- Handle errors properly (don't unwrap carelessly)

### General

- Keep code DRY (Don't Repeat Yourself)
- Write self-documenting code
- Add comments for complex logic
- Optimize for readability over cleverness
- Consider performance implications

## Testing

### Manual Testing

Test your changes on your platform:
- Open videos with different formats and codecs
- Test keyboard shortcuts
- Test drag and drop functionality
- Test file associations (if applicable)
- Test with different video aspect ratios

### Cross-Platform Considerations

If possible, test on multiple platforms:
- Windows 10/11
- macOS 12+
- Ubuntu/Debian-based Linux

Document any platform-specific behavior or limitations.

## Documentation

### Code Documentation

- Add JSDoc/rustdoc comments for public APIs
- Explain "why" not just "what" in comments
- Update existing documentation when changing behavior

### User Documentation

Update README.md or docs when:
- Adding new features
- Changing keyboard shortcuts
- Modifying configuration options
- Changing build requirements

## Community

### Getting Help

- **Discussions**: Use GitHub Discussions for questions
- **Issues**: Check existing issues before creating new ones
- **Discord/Chat**: (Add link if available)

### Feature Requests

Before implementing a major feature:
1. Open an issue to discuss the feature
2. Wait for feedback from maintainers
3. Get approval before starting work

This prevents wasted effort on features that may not be accepted.

### Bug Reports

When reporting bugs:
- Use the bug report template
- Include steps to reproduce
- Provide system information
- Include logs and screenshots
- Test on latest version first

## Recognition

Contributors will be:
- Listed in the project README
- Mentioned in release notes
- Credited in commit history

Thank you for contributing to glucose! 🎬
