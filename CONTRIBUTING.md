# Contributing

## Local Development

Install the binary from the current checkout with:

```command
cargo install --path .
```

Then run `claude-history` from the same directory where you previously ran `claude`.

## Releases

This repository uses `release-plz` from GitHub Actions.

`release-plz` owns versioning, changelogs, crates.io publishing, tags, and GitHub releases. A separate GoReleaser workflow runs after a GitHub release is published and uploads binary artifacts for Linux, macOS, and Windows to that existing release.

To enable automated releases in GitHub:

1. In the repository Actions settings, allow workflows to create and approve pull requests.
2. Add a `CARGO_REGISTRY_TOKEN` repository secret with permission to publish updates to crates.io.
3. Optionally add a `RELEASE_PLZ_TOKEN` repository secret if you want CI workflows to run on release PRs opened by `release-plz`. Without it, `release-plz` will fall back to the default `GITHUB_TOKEN`, and GitHub will not trigger other workflows from that PR.

After that, pushes to `main` will keep a release PR up to date, and merging that PR will publish the crate and create the GitHub release.
