---
name: release
description: Guide the release process for swaybeam, including version bumping, changelog updates, and creating release branches.
---

## Purpose

Provide step-by-step instructions for releasing a new version of swaybeam, ensuring proper versioning, changelog updates, and release commit management.

## When to use

Use this skill when asked to:
- Create a new release
- Bump the version
- Update the changelog for a release
- Prepare a release commit

## Prerequisites

Before starting a release:
1. Ensure you are on the `main` branch
2. Ensure the working tree is clean (no uncommitted changes)
3. Ensure local main is up to date with origin/main
4. Ensure HEAD is at origin/main (no unpushed commits)

## Version Decision Guide

Use Semantic Versioning (MAJOR.MINOR.PATCH). Determine the bump type by analyzing commits since the last release:

### Major Version (X.0.0)

Bump MAJOR when:
- Breaking changes to the CLI interface (removed/renamed commands or flags)
- Breaking changes to configuration format
- Breaking changes to public APIs
- Commit message contains `BREAKING CHANGE:` or `!` (e.g., `feat!: ...`)

### Minor Version (0.X.0)

Bump MINOR when:
- New features added (`feat:` commits)
- New CLI commands or flags
- New configuration options
- Backward-compatible enhancements

### Patch Version (0.0.X)

Bump PATCH when:
- Bug fixes (`fix:` commits)
- Documentation updates (`docs:` commits)
- Internal refactoring (`refactor:` commits)
- Performance improvements without API changes
- Dependency updates

### Decision Process

1. Run: `git log v$(sed -n 's/^version = "\(.*\)"/\1/p' ./Cargo.toml | head -n1)..HEAD --oneline`
2. Check commit messages for:
   - `!` or `BREAKING CHANGE:` -> MAJOR
   - `feat:` -> MINOR
   - `fix:`, `docs:`, `refactor:`, etc. -> PATCH
3. If multiple types, use the highest precedence (MAJOR > MINOR > PATCH)

## Release Process

### Step 1: Verify Clean State and Sync

Ensure HEAD is at origin/main with no uncommitted or unpushed changes:

```bash
git checkout main
git pull origin main
git status  # Should show "nothing to commit, working tree clean"
```

Check for unpushed commits:

```bash
git rev-list --count origin/main..HEAD  # Should output 0
```

If there are local commits not on origin/main, they must be merged first. The changelog template needs the main commit ID.

### Step 2: Determine Version

1. Get current version:
   ```bash
   grep '^version =' Cargo.toml
   ```

2. Review commits since last release:
   ```bash
   git log v<CURRENT_VERSION>..HEAD --oneline
   ```

3. Decide on MAJOR, MINOR, or PATCH bump based on the Version Decision Guide above.

### Step 3: Create Release Branch

Create a branch named `release/v{NEW_VERSION}`:

```bash
git checkout -b release/v<NEW_VERSION>
```

Example: `git checkout -b release/v0.2.0`

### Step 4: Update Version in Cargo.toml

Edit `Cargo.toml` and update the version field in the `[workspace.package]` section:

```toml
version = "<NEW_VERSION>"
```

### Step 5: Update Dependencies

Run the update-version just target to update version references and Cargo.lock:

```bash
just update-version
```

This command:
- Updates version references in workspace Cargo.toml files
- Runs `cargo update --workspace`

### Step 6: Update Changelog

Generate the changelog using git-cliff with the version from Cargo.toml:

```bash
just update-changelog
```

This runs: `git-cliff --config cliff.toml --unreleased --tag v<VERSION> -o CHANGELOG.md`

The changelog will be automatically updated with:
- A new versioned section for the current version
- All commits since the last tag, grouped by type (Added, Fixed, Documentation, etc.)
- Previous version history preserved (v0.1.2, v0.1.1, etc.)

**Important**: The `--unreleased --tag v<VERSION>` flags ensure the changelog shows the proper version instead of `[unreleased]`.

### Step 7: Commit Changes

Stage and commit all changes:

```bash
git add .
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' ./Cargo.toml | head -n1)
git commit -m "release: Version $VERSION"
```

### Step 8: Push Branch and Create PR

Push the release branch:

```bash
git push -u origin release/v<NEW_VERSION>
```

Create a pull request to merge into main.

### Step 9: After Merge

After the commit is merged to main:
1. Create and push a tag: `git tag v<VERSION> && git push origin v<VERSION>`
2. The CI workflow automatically builds release artifacts and creates a GitHub Release
3. The AUR publish workflow automatically publishes to AUR

## Quick Reference

| Step | Command |
|------|---------|
| Check current version | `grep '^version =' Cargo.toml` |
| View recent commits | `git log v<CUR>..HEAD --oneline` |
| Check unpushed commits | `git rev-list --count origin/main..HEAD` |
| Create branch | `git checkout -b release/v<VER>` |
| Update version refs | `just update-version` |
| Update changelog | `just update-changelog` |
| Commit | `git commit -m "release: Version <VER>"` |
| Push branch | `git push -u origin release/v<VER>` |

## Checklist

- [ ] On main branch, clean working tree
- [ ] Pulled latest from origin/main
- [ ] No unpushed commits (HEAD at origin/main)
- [ ] Determined version bump type (MAJOR/MINOR/PATCH)
- [ ] Created release branch `release/v<VERSION>`
- [ ] Updated version in Cargo.toml
- [ ] Ran `just update-version`
- [ ] Ran `just update-changelog`
- [ ] Committed with message `release: Version <VERSION>`
- [ ] Pushed branch and created PR
- [ ] After merge: created tag `v<VERSION>`
