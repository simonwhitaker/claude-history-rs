# Claude History

Get a transcript of your last Claude Code session, ready to include in your commit message.

## Installation

```command
cargo install --path .
```

Then run `claude-history` from the same directory where you previously ran `claude`.

## Usage

To get a list of command-line options:

```command
claude-history --help
```

## Releases

This repository is configured to use `release-plz` from GitHub Actions.

To enable automated releases in GitHub:

1. In the repository Actions settings, allow workflows to create and approve pull requests.
2. Add a `CARGO_REGISTRY_TOKEN` repository secret with permission to publish updates to crates.io.
3. Optionally add a `RELEASE_PLZ_TOKEN` repository secret if you want CI workflows to run on release PRs opened by `release-plz`. Without it, `release-plz` will fall back to the default `GITHUB_TOKEN`, and GitHub will not trigger other workflows from that PR.

After that, pushes to `main` will keep a release PR up to date, and merging that PR will publish the crate and create the GitHub release.
