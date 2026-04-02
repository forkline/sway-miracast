# swaybeam Release Skill

This skill provides a standardized process for creating releases for the swaybeam project.

## Purpose

The release skill standardizes the swaybeam release process to:
- Ensure consistent versioning and tagging
- Validate changes against project guidelines
- Generate proper changelog entries
- Perform pre-release checks
- Verify post-release artifacts

## Release Process

### 1. Pre-checks

Before initiating a release, verify:

- All tests pass: `cargo test`
- Code is formatted: `cargo fmt`
- No lint warnings: `cargo clippy --workspace --all-targets`
- Changelog updates are comprehensive and reflect all changes

### 2. Version Determination

1. Analyze Git history since last tag using conventional commits:

   - `feat:` commits increment minor version
   - `fix:` commits increment patch version
   - Breaking changes (`!` or `BREAKING CHANGE:`) increment major version

2. Use semantic versioning (MAJOR.MINOR.PATCH):
   - MAJOR for breaking changes
   - MINOR for backward-compatible features
   - PATCH for backward-compatible fixes

### 3. Update Version Files

1. Update `Cargo.toml` in workspace root and all relevant packages with new version
2. Run `cargo update -p swaybeam` to update `Cargo.lock`
3. Run `just update-changelog` to generate changelog entries
4. Commit version changes

### 4. Create GitHub Release

1. Push the version commit to main branch
2. Create Git tag: `git tag -a vx.y.z -m "Release vx.y.z"`
3. Push tag: `git push origin vx.y.z`
4. Create GitHub release using tag with changelog content
5. Verify published crate integrity

### 5. Post-relase

- Update any documentation affected by the release
- Announce release in relevant channels if applicable

## Release Checklist

Each release must satisfy:

- [ ] All tests pass including integration tests
- [ ] No regressions in core functionality
- [ ] Changelog accurately reflects all changes
- [ ] Version numbers updated in all relevant files
- [ ] Documentation is consistent with changes
- [ ] Cargo.lock updated with `cargo update -p swaybeam`
- [ ] Release builds successfully with `cargo build --release`
- [ ] Published artifacts contain expected content
