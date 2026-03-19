# Claude History: guidance for coding agents

- Use conventional commits when commiting code
- Use `actionlint` to lint GitHub Actions workflow files after editing them
- Keep the GoReleaser workflow on a macOS runner while building Apple artifacts; the macOS targets need Xcode command line tools on the host
- `main` is protected, so push changes on a `simon/*` branch and use a pull request instead of trying to push directly
