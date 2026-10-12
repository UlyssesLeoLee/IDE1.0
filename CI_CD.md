# CI/CD Documentation for IDE1.0

## Overview

This document describes the complete CI/CD pipeline for IDE1.0, including GitHub Actions and Jenkins configurations.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        CI/CD Pipeline                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐        │
│  │   GitHub     │   │   Jenkins    │   │   Local Dev  │        │
│  │   Actions    │   │   Pipeline   │   │   (pre-commit)│        │
│  └──────┬───────┘   └──────┬───────┘   └──────┬───────┘        │
│         │                  │                  │                 │
│         ▼                  ▼                  ▼                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    Quality Gates                          │  │
│  │  fmt │ clippy │ test │ audit │ deny │ markdown │ parity  │  │
│  └──────────────────────────────────────────────────────────┘  │
│         │                  │                  │                 │
│         ▼                  ▼                  ▼                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Build Matrix (Parallel)                      │  │
│  │  Linux (deb, AppImage) │ Windows (msi) │ macOS (dmg)      │  │
│  └──────────────────────────────────────────────────────────┘  │
│         │                  │                  │                 │
│         ▼                  ▼                  ▼                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Integration & UAT                            │  │
│  │  cli_smoke │ Playwright e2e │ Mock Switch │ Parity        │  │
│  └──────────────────────────────────────────────────────────┘  │
│         │                  │                  │                 │
│         ▼                  ▼                  ▼                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    Release                                │  │
│  │  Artifacts │ Checksums │ GitHub Release │ Artifactory     │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## GitHub Actions Workflow (`.github/workflows/ci.yml`)

### Jobs Overview

| Job | Platform | Purpose | Triggers |
|-----|----------|---------|----------|
| `rust` | ubuntu, windows, macOS (stable/beta/nightly) | Code quality, build, test | push, PR |
| `integration` | ubuntu | System tests + cli_smoke | push, PR |
| `mock-switch-validate` | ubuntu | Python mock tests | push, PR |
| `uat-playwright` | ubuntu | Playwright e2e (40 cases) | push, PR |
| `cross-language-parity` | ubuntu | Rust ↔ Python parity | push, PR |
| `markdown-lint` | ubuntu | Markdown linting | push, PR |
| `tauri-build-linux` | ubuntu | .deb + .AppImage | push, release |
| `tauri-build-windows` | windows | .msi | push, release |
| `tauri-build-macos` | macos | .dmg | push, release |
| `rust-bench` | ubuntu | Performance benchmarks | push, schedule |
| `security-audit` | ubuntu | cargo-audit + cargo-deny | push, PR, schedule |
| `cross-project-smoke` | ubuntu | aci-emitter git dep | push, PR |
| `release` | ubuntu | GitHub Release | release published |

### Matrix Strategy

```yaml
strategy:
  fail-fast: false
  matrix:
    os: [ubuntu-latest, windows-latest, macos-latest]
    rust: [stable]
    include:
      - os: ubuntu-latest
        rust: beta
      - os: ubuntu-latest
        rust: nightly
        allow-failure: true
```

### Key Features

1. **Multi-platform builds** - Linux/Windows/macOS in parallel
2. **Rust channel testing** - stable + beta + nightly (nightly allow-failure)
3. **Artifact caching** - Swatinem/rust-cache for fast builds
3. **Platform-specific deps** - System deps per OS
4. **Test isolation** - Temp directories per test run
5. **Artifact archiving** - All binaries + test reports
6. **Security scanning** - cargo-audit + cargo-deny weekly

## Jenkins Pipeline (`Jenkinsfile`)

### Pipeline Structure

```groovy
pipeline {
    agent none
    
    stages {
        stage('Checkout & Setup') { ... }
        stage('Code Quality') {      // Parallel: fmt+clippy, markdown, security
            parallel { ... }
        }
        stage('Build & Test') {      // Parallel: Linux, Windows, macOS
            parallel { ... }
        }
        stage('Integration Tests') { ... }
        stage('UAT (Playwright)') { ... }
        stage('Mock Switch Validation') { ... }
        stage('Cross-language Parity') { ... }
        stage('Tauri Desktop Builds') {  // Parallel: Linux, Windows, macOS
            parallel { ... }
        }
        stage('Performance Benchmarks') { ... }
        stage('Release') { when { tag 'v*' } }
    }
}
```

### Agent Requirements

| Label | Purpose | Required Tools |
|-------|---------|----------------|
| `rust` | Rust builds | rust, cargo, rustfmt, clippy |
| `linux && rust` | Linux builds | + gtk3, webkit2gtk, xvfb |
| `windows && rust` | Windows builds | + chocolatey, wixtoolset |
| `macos && rust` | macOS builds | + homebrew, create-dmg |
| `node` | Node.js tasks | node 20, npm |
| `python` | Python tasks | python 3.11, pytest |
| `rust && python` | Cross-lang parity | Both Rust + Python |

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `TARGET_PLATFORM` | choice | `all` | `all`, `linux`, `windows`, `macos` |
| `SKIP_TESTS` | boolean | `false` | Emergency skip |
| `PUBLISH_ARTIFACTS` | boolean | `true` | Publish to artifact repo |
| `RELEASE_VERSION` | string | `` | Version for release |

## Configuration Files

| File | Purpose |
|------|---------|
| `.github/workflows/ci.yml` | GitHub Actions workflow |
| `Jenkinsfile` | Jenkins Declarative Pipeline |
| `.markdownlint.json` | Markdown lint rules |
| `.cargo-deny.toml` | Cargo deny configuration |
| `tests/uat/package.json` | Playwright UAT config |
| `tools/package.json` | Tauri CLI dependency |

## Quality Gates

All must pass for merge:

| Gate | Tool | Threshold |
|------|------|-----------|
| Formatting | `cargo fmt --check` | Zero diff |
| Linting | `cargo clippy -D warnings` | Zero warnings |
| Unit tests | `cargo test --workspace` | 100% pass |
| Integration | `cargo test --tests` | 100% pass |
| UAT | Playwright | 40/40 pass |
| Mock switch | Python pytest | 26/26 pass |
| Cross-lang parity | Python script | 100% match |
| Security audit | cargo-audit | Zero vulnerabilities |
| License check | cargo-deny | Approved licenses only |
| Markdown lint | markdownlint-cli2 | Zero errors |

## Artifacts

### Build Artifacts (per platform)

| Platform | Artifacts |
|----------|-----------|
| Linux | `ide-shell`, `ide-shell-web`, `ide-shell-desktop` + `.deb`, `.AppImage` |
| Windows | `ide-shell.exe`, `ide-shell-web.exe`, `ide-shell-desktop.exe` + `.msi` |
| macOS | `ide-shell`, `ide-shell-web`, `ide-shell-desktop` + `.dmg` |

### Test Reports

| Report | Location |
|--------|----------|
| Playwright HTML | `tests/uat/playwright-report/` |
| JUnit XML | `tests/uat/test-results/` |
| Rust test output | Console + artifacts |
| Benchmark output | Console |

## Release Process

1. **Tag push**: `git tag v0.1.9 && git push origin v0.1.9`
2. **GitHub Actions**: Runs full pipeline including all platform builds
3. **Release job**: Downloads all artifacts, generates checksums
4. **GitHub Release**: Auto-created with assets + checksums

```bash
# Local release prep
git tag v0.1.9
git push origin v0.1.9
# CI handles the rest
```

## Local Development

### Pre-commit Hook (Recommended)

```bash
# Install
cargo install cargo-husky
cargo husky install

# .husky/pre-commit
#!/bin/sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --no-deps -- -D warnings
cargo test --workspace --lib
```

### Local CI Simulation

```bash
# Run all quality gates locally
./scripts/ci-local.sh

# Or manually:
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --no-deps -- -D warnings
cargo test --workspace --all-targets --no-fail-fast
cargo audit
cargo deny check licenses
cargo deny check bans
```

## Troubleshooting

### Common Issues

| Issue | Solution |
|-------|----------|
| `cargo cache` corrupted | `cargo clean && rm -rf ~/.cargo/registry/cache` |
| Tauri build fails on Linux | Install all GTK/WebKit deps: `apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev ...` |
| Windows .msi build fails | Install WiX Toolset: `choco install wixtoolset` |
| macOS .dmg build fails | Install create-dmg: `brew install create-dmg` |
| Playwright tests flaky | Run with `--retries=2`, use `xvfb-run` on Linux |
| cargo-deny license failures | Add exception in `.cargo-deny.toml` with reason |

### Cache Issues

```bash
# Clear all caches
cargo clean
rm -rf ~/.cargo/registry/cache
rm -rf ~/.cargo/git/db
```

## Maintenance

### Weekly (Automated)
- Security audit (Monday 2AM)
- Dependency updates check

### Monthly
- Review cargo-deny exceptions
- Update Rust toolchain versions
- Review Playwright test stability

### Per Release
- Update version in `Cargo.toml` workspace
- Update `tauri.conf.json` version
- Generate changelog from commit history
- Verify all platform artifacts

---

*Last updated: 2026-10-10*
*Pipeline version: 2.0 (GitHub Actions + Jenkins)*