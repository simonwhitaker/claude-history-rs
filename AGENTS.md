# Claude History: guidance for coding agents

- Use conventional commits when commiting code
- Use `actionlint` to lint GitHub Actions workflow files after editing them
- Keep the GoReleaser workflow on a macOS runner while building Apple artifacts; the macOS targets need Xcode command line tools on the host
- Before opening a PR, run the relevant local validation for the change and make sure it passes
- For Rust code changes, run `cargo fmt`, `cargo fmt --check`, and `cargo test` before opening a PR
- PR titles must use conventional commit style because the repository uses squash merges and the PR title becomes the squashed commit message
