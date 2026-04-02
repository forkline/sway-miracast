# CI/CD Workflows Documentation

This document describes the GitHub Actions workflows used in sway-miracast.

## Workflows Overview

| Workflow | Purpose | Trigger |
|----------|---------|---------|
| `rust.yml` | Main CI pipeline | Push to main, PRs, tags |
| `pre-commit.yml` | Pre-commit checks | Push to main, PRs |
| `auto-tag.yaml` | Automatic version tagging | Push to main |
| `stale.yml` | Stale issue/PR management | Daily schedule |

## Rust Workflow (`rust.yml`)

The main CI pipeline that runs on every push and pull request.

### Jobs

1. **fmt** - Checks code formatting with `rustfmt`
2. **clippy** - Lints code with `clippy`
3. **test** - Runs all tests
4. **docs** - Builds documentation
5. **build** - Builds release artifacts

### Release Process

When a tag matching `v[0-9]*` is pushed:

1. Builds release binary
2. Creates tarball with binary
3. Generates SHA256 checksum
4. Extracts changelog entry
5. Creates GitHub Release with:
   - Release notes from CHANGELOG.md
   - Binary tarball
   - SHA256 checksum file

## Auto Tag Workflow (`auto-tag.yaml`)

Automatically creates signed Git tags when CHANGELOG.md is updated with a new version.

### Setup Requirements

1. **Create Personal Access Token (PAT)**
   ```bash
   # Go to GitHub → Settings → Developer settings → Personal access tokens
   # Create token with 'repo' scope
   ```

2. **Generate GPG Key**
   ```bash
   gpg --full-generate-key
   # Choose: RSA and RSA, 4096 bits, key does not expire
   # Enter your name and email

   # Export the key
   gpg --armor --export-secret-keys YOUR_EMAIL > gpg-private-key.asc

   # Get your key ID
   gpg --list-keys --with-colons | grep '^pub' | cut -d':' -f5
   ```

3. **Add Public Key to GitHub**
   ```bash
   gpg --armor --export YOUR_EMAIL | pbcopy
   # Go to GitHub → Settings → SSH and GPG keys → New GPG key
   # Paste the public key
   ```

4. **Add Repository Secrets**
   - `PAT` - Your Personal Access Token
   - `GPG_PRIVATE_KEY` - Contents of `gpg-private-key.asc`

### How It Works

1. Triggered on push to `main` branch
2. Reads latest version from `CHANGELOG.md`
3. Checks if tag already exists
4. If not, creates signed tag and pushes it
5. Tag triggers `rust.yml` which creates the release

## Pre-commit Workflow (`pre-commit.yml`)

Runs pre-commit hooks on all files.

### Hooks Configured

- `check-added-large-files`
- `check-executables-have-shebangs`
- `check-merge-conflict`
- `check-shebang-scripts-are-executable`
- `detect-private-key`
- `end-of-file-fixer`
- `mixed-line-ending`
- `trailing-whitespace`
- `yamllint`
- `cargo fmt` (skipped, runs in rust.yml)
- `cargo clippy` (skipped, runs in rust.yml)

## Stale Workflow (`stale.yml`)

Automatically manages stale issues and pull requests.

### Behavior

- **Days before stale**: 30
- **Days before close**: 7
- **Exempt labels**: `pinned`, `security`, `enhancement`

### Schedule

Runs daily at 00:00 UTC.

## Local Development

### Pre-commit Setup

```bash
# Install pre-commit
pip install pre-commit

# Install hooks
just pre-commit-install
# or
pre-commit install

# Run manually
just pre-commit
# or
pre-commit run --all-files
```

### Running CI Locally

```bash
# Format check
just fmt-check

# Clippy
just clippy

# All tests
just test

# Full lint
just lint
```

### Creating a Release

1. **Update Version**
   ```bash
   ./ci/release.sh
   # or manually:
   vim Cargo.toml  # Update version
   cargo update -p sway-miracast
   just update-changelog
   git add .
   git commit -m "release: Version X.Y.Z"
   ```

2. **Create PR and Merge**
   ```bash
   git push origin HEAD:release/vX.Y.Z
   # Create PR, review, merge
   ```

3. **Automatic Tag and Release**
   - After merge, `auto-tag.yaml` creates the tag
   - Tag triggers `rust.yml` which creates the release

## Secrets Required

| Secret | Workflow | Purpose |
|--------|----------|---------|
| `GITHUB_TOKEN` | All | Auto-provided by GitHub |
| `PAT` | auto-tag.yaml | Personal Access Token for tagging |
| `GPG_PRIVATE_KEY` | auto-tag.yaml | GPG key for signing tags |

## Workflow Files

```
.github/
└── workflows/
    ├── rust.yml          # Main CI pipeline
    ├── pre-commit.yml    # Pre-commit checks
    ├── auto-tag.yaml     # Automatic tagging
    └── stale.yml         # Stale management
```

## Just Commands

```bash
just --list              # Show all commands
just build              # Build release
just test               # Run tests
just lint               # Format + clippy
just lint-fix           # Auto-fix lint issues
just pre-commit         # Run pre-commit
just release            # Create release tarball
just update-changelog   # Update CHANGELOG.md
```

## Troubleshooting

### Pre-commit Fails

```bash
# Clear cache
pre-commit clean

# Reinstall hooks
pre-commit install --install-hooks

# Run with verbose output
pre-commit run --all-files --verbose
```

### GPG Key Issues

```bash
# List keys
gpg --list-keys

# Test signing
echo "test" | gpg --clearsign

# Export public key
gpg --armor --export YOUR_EMAIL
```

### Release Not Created

1. Check if tag was created: `git tag -l`
2. Check GitHub Actions logs
3. Verify `CHANGELOG.md` has correct version format
4. Ensure `PAT` has `repo` scope

## Best Practices

1. **Always run `just lint` before committing**
2. **Update CHANGELOG.md with significant changes**
3. **Use conventional commit messages**
4. **Review CI failures promptly**
5. **Keep PRs focused and small**