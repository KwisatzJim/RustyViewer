# Contributing to RustyViewer

Bug reports, focused improvements and documentation fixes are welcome.

## Report a problem

Search existing issues first, then use the bug-report form. Include your RustyViewer
version, operating system, image format and exact steps. Please use a disposable
sample image or a small generated fixture; do not upload a private photograph just
to demonstrate a bug.

Security issues should follow [SECURITY.md](SECURITY.md) instead of a public issue.

## Make a change

1. Fork the repository and create a focused branch.
2. Keep file and export operations in Rust. The frontend should request them through
   a narrow Tauri command.
3. Preserve Batch studio's no-overwrite behavior and the unexported-change warning.
4. Add a regression test when fixing image data, history, validation or file safety.
5. Format and run the checks below.
6. Open a pull request describing the user-visible behavior and your validation.

```sh
npm ci
npm run check
npm run format:check
npm run version:check
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Changes to buttons, dialogs, selection, drag/drop, clipboard handling or shortcuts
also need a hands-on check in a real Tauri window. Keep pull requests manageable;
one well-explained improvement is easier to review than unrelated changes together.

