# Contributing

Thanks for helping make Sailwind Save Manager safer. Data integrity takes priority over convenience and performance.

## Before opening a change

Use a GitHub issue for behavior changes or anything that modifies the snapshot, archive or restore formats. Security vulnerabilities should follow [SECURITY.md](./SECURITY.md) instead of a public issue.

Keep pull requests focused. Explain the user impact and any filesystem-safety implications.

## Local checks

Install Node.js 22 or newer, Rust stable and the platform dependencies required by Tauri 2. Then run:

```sh
npm ci
npm audit --audit-level=high
npm run check
npm test
npm run build
cd src-tauri
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Changes to restore, import, export, deletion or retention behavior need regression tests using temporary directories. Tests must prove both the expected result and preservation of the original save data on failure.

## Design constraints

- Do not follow symbolic links or junction-like redirects found in save bundles or imported archives.
- Keep snapshot payloads and manifests immutable.
- Treat SQLite as a rebuildable index, not the source of truth.
- Keep complete filesystem transactions inside Rust commands.
- Do not add telemetry or transmit save data.
- Do not use `unsafe` Rust without an accepted design decision and a dedicated audit.

## Pull requests

CI checks formatting, tests, strict Clippy, dependency audit and a real Windows installer build. Download the Windows artifact and complete the relevant sections of the [alpha test checklist](./docs/ALPHA_TEST_CHECKLIST.md) before marking a release-oriented PR ready.
