# Sailwind Save Manager

A lightweight, offline-first desktop application for creating and restoring verified Sailwind save backups.

The project is currently an early MVP. It handles raw save bundles without decoding or editing Sailwind's main `.save` payload.

## Current capabilities

- Windows save-directory detection and experimental Steam Deck/Proton detection
- manual directory selection with a native picker
- discovery of complete slot bundles, including metadata, thumbnails, rotating game backups and mod sidecars
- immutable snapshots with ULIDs, SHA-256 checksums and versioned manifests
- durable labels, notes and retention protection outside the rebuildable SQLite index
- verified restore with a safety snapshot, staging, rollback and interrupted-operation recovery
- restore blocking while `Sailwind.exe` is running
- protected and recoverable snapshot deletion
- `.swbackup` ZIP import/export with traversal, symlink, duplicate-entry and resource-limit checks
- a 50-snapshot retention threshold that never removes protected snapshots without user control

## Development

Requirements:

- Node.js 22 or newer
- Rust stable
- platform dependencies required by Tauri 2

Install and verify:

```sh
npm install
npm run check
npm test
cd src-tauri
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Run the desktop application:

```sh
npm run tauri dev
```

Create an optimized local build:

```sh
npm run tauri build
```

The current Tauri configuration produces an executable without an installer while distribution and signing workflows are being finalized.

## Safety model

The frontend collects user decisions, while complete filesystem-changing operations execute inside individual Rust commands. Snapshot payloads and manifests are immutable. SQLite is only an index and can be rebuilt from snapshot manifests and annotation files.

Imported archives and all discovered save artifacts are treated as untrusted input. Symbolic links are not followed.

See [plan-16.md](./plan-16.md) for the accepted product and architecture decisions. The [deep research report](./deep-research-report.md) is supporting research rather than the active implementation specification.

## Author

hiosdra

## License

GNU Affero General Public License v3.0 only. See [LICENSE](./LICENSE).
