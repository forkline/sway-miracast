#!/bin/bash
set -e

if ! [ "$(git rev-list --count origin/main..HEAD)" -eq 0 ]; then
    echo "There are commits in this branch. Please merge them first."
    echo "CHANGELOG template needs main commit ID."
    exit 1
fi

# bump version
vim ./Cargo.toml

just update-version

just update-changelog

# Ensure CHANGELOG has trailing newline
sed -i -e '$a\' CHANGELOG.md

git add .
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' ./Cargo.toml | head -n1)
git commit -m "release: Version $VERSION"

echo "After merging the PR, tag and release are automatically done"
