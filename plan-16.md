# Sailwind Save Manager — plan

> Living decision document. Only confirmed facts and decisions explicitly accepted by the user are treated as settled. Open questions remain marked as such.

## Product goal

Create a modern desktop application for managing Sailwind save files, focused on:

- easy installation for end users;
- substantially lower runtime overhead than Electron;
- an approachable codebase for the author and outside contributors;
- TypeScript where it is a sensible fit;
- safe local save backup and version history;
- optional backup to a GitHub repository created or selected by the user.

Project metadata:

- application name: **Sailwind Save Manager**;
- author: **hiosdra**;
- application identifier: **`io.github.hiosdra.sailwind-save-manager`**.

## Implementation status

As of 2026-07-21, the first local-backup vertical slice is implemented and covered by automated tests:

- Tauri 2, Svelte 5, Vite and TypeScript strict application shell;
- automatic standard-path detection and native manual directory selection;
- complete slot-bundle discovery without following symbolic links;
- immutable snapshots, SHA-256 manifests, durable annotations and a rebuildable SQLite index;
- restore with game-process blocking, safety snapshot, staging, rollback and interrupted-operation recovery;
- labels, retention protection, deletion and the 50-snapshot retention confirmation flow;
- hardened `.swbackup` import and export;
- privacy-safe JSONL operation history, a copyable diagnostic report and a persistent recovery notice;
- frontend checks/tests, Rust tests, strict Clippy, CI and a successful optimized local build;
- Windows NSIS/MSI/portable artifact builds for pull requests and a manual draft-prerelease workflow;
- alpha test, contribution, security, changelog and roadmap documentation.

Still pending from the broader MVP/release plan are polished UI design, a dedicated notes editor, opening data directories, update notification, a reviewable diagnostic ZIP, Playwright desktop workflows, code signing and the fully automated nightly/stable release channels described below. Windows packaging must also pass its first hosted CI run and the manual alpha checklist before it is considered release-validated.

## Confirmed domain facts

### Save locations

- Standard Windows location:
  `%USERPROFILE%\\AppData\\LocalLow\\Raw Lion Workshop\\Sailwind`
- Steam Deck / Proton location follows the Windows path inside Steam's Proton prefix for app `1764530`.
- The `PortableSaves` mod may redirect saves to:
  `<Sailwind installation>/Sailwind_Data/Saves`
- The application must support manual selection of a save directory as a fallback.

### Save bundle

A slot must be treated as a bundle rather than only one file. Recognized artifacts may include:

- `slotN.save`
- `slotN.save.meta`
- `slotN.save.png`
- `slotN_backup1.save` … `slotN_backup5.save`
- optional `slotN/` sidecar directory used by mods

Backup, restore, copy and deletion operations should handle the entire detected bundle atomically.

### Save format and compatibility

- The save exposes structured fields such as currencies, reputation, survival stats, player position, ships and item instances.
- The exact byte-level codec has not yet been conclusively verified from a primary source.
- The implementation must separate:
  1. raw byte handling;
  2. codec detection and decoding;
  3. generic save-tree representation;
  4. schema-aware features.
- Unknown fields must be preserved losslessly.
- A raw backup must be created before every write operation.
- Sailwind is in Early Access, so schema drift must be expected.

## Confirmed product principles

- Offline-first operation.
- Local backup/version history is a core feature.
- GitHub integration is optional and must never block local backup or restore.
- The application should provide safe, purpose-built editing workflows instead of relying only on a generic tree editor.
- Advanced/raw editing may be provided, but should be clearly separated from safer operations.

## Candidate high-value editing features

These are research-backed candidates, not yet committed to a release milestone:

- currencies;
- regional reputation;
- food, water and sleep state;
- in-game time;
- player relocation / teleport to known locations;
- ship recovery / unsinking;
- ship relocation;
- searchable item-instance browser;
- generic structured save-tree inspector.

## Open decisions

### D1. Desktop application stack — DECIDED

**Tauri 2 + TypeScript frontend + Rust native core**

Responsibility split:

- TypeScript: UI, forms, view models, presentation of diffs and user-facing validation;
- Rust: save path detection, file watching, bundle discovery, atomic writes, snapshots, restore operations, codec handling and secure native integrations;
- the boundary between TypeScript and Rust should remain narrow and use typed Tauri commands/events.

Rationale:

- lower distribution and runtime overhead than Electron;
- system WebView rather than bundled Chromium;
- TypeScript-friendly UI and contributor experience;
- Rust is well suited to safety-critical file operations and future save codec work;
- mature desktop packaging and updater support.

### D2. Frontend framework — DECIDED

**Svelte 5 + Vite + TypeScript strict**

Frontend test stack:

- Vitest for unit and component-level tests;
- Playwright for end-to-end desktop UI workflows.

Initial frontend scope intentionally excludes SvelteKit, a global state library and a dedicated router. These may be introduced later only if concrete requirements justify them.

### D3. Supported operating systems for the first release — DECIDED

- Windows: officially supported for the first release;
- Linux / Steam Deck: architecturally supported from the start, but treated as experimental initially;
- macOS: out of scope for the first release.

### D4. Local history implementation — DECIDED

**Immutable snapshot payloads + durable annotations + SQLite as a rebuildable metadata index**

Rules:

- each snapshot contains a full copy of the detected save bundle;
- snapshots are immutable after creation;
- each snapshot contains an immutable `manifest.json` with file checksums and core metadata;
- editable labels, notes and retention protection are stored as atomic per-snapshot annotation files outside the immutable snapshot directory;
- SQLite stores searchable metadata and UI-oriented indexes derived from manifests and annotation files;
- SQLite is not the source of truth and can be rebuilt from snapshot manifests and annotations;
- snapshot creation uses a staging directory followed by an atomic rename;
- restore always creates a safety snapshot of the current save before overwriting it;
- deduplication is deferred until measurements show it is necessary.


### D7. Snapshot format and retention — DECIDED

- snapshots use ULIDs as internal identifiers and timestamps for display;
- every snapshot is immutable and contains the complete detected slot bundle;
- `manifest.json` stores core metadata, file sizes and SHA-256 checksums for every file;
- labels, notes and protection state are stored in atomic per-snapshot annotation files and indexed in SQLite without modifying snapshot contents;
- export uses `.swbackup`, implemented as a standard ZIP archive containing the manifest and full bundle;
- the retention threshold is 50 snapshots per slot rather than a hard storage limit;
- at the threshold the user chooses whether to prune the oldest unprotected snapshots automatically or require confirmation each time; confirmation is the default;
- a snapshot may still be created when every existing snapshot is protected, with a clear warning that the threshold has been exceeded;
- protected snapshots are excluded from automatic deletion;
- automatic safety snapshots are created before restore and before future save-modifying operations.


### D8. First implementation vertical slice — DECIDED

**Full local backup flow**

The first implementation slice includes:

- automatic or manual save directory selection;
- slot and full bundle discovery;
- slot list in the frontend;
- snapshot creation with manifest and checksums;
- SQLite metadata indexing;
- snapshot history;
- restore;
- automatic safety snapshot before restore.

Import/export and automatic retention enforcement follow immediately after this slice, but are not required for the first end-to-end implementation milestone.


### D10. Rust–TypeScript responsibility boundary — DECIDED

**Rust-enforced filesystem transactions with presentation workflow in TypeScript**

Rust owns native primitives and every complete workflow that changes save or backup files:

- filesystem access and platform path detection;
- atomic copy, staging and rename operations;
- checksum calculation;
- SQLite access and migrations;
- archive import/export primitives;
- snapshot creation, restore, import and deletion as individually auditable Tauri commands;
- operation locking, staging, verification, journaling and crash recovery inside those commands.

TypeScript owns presentation workflow and user-facing decisions:

- retention decisions and warnings;
- view state, filtering and sorting;
- confirmation flows and presentation validation;
- mapping native results into UI models.

The frontend must not compose a safety-critical filesystem transaction from multiple native calls. For example, verified safety-snapshot creation and live-bundle replacement are one Rust restore operation after the frontend has collected confirmation. The Rust API should remain small, explicit and easy to audit. Rust code should use no `unsafe`, pass `cargo fmt` and strict `clippy`, avoid advanced macros and speculative abstractions, and be covered by integration tests using temporary directories.

### D11. SQLite access — DECIDED

**`rusqlite` with explicit SQL migrations**

The Rust persistence layer will use direct, reviewable SQL rather than an ORM. Database migrations will be stored as versioned SQL files under `src-tauri/migrations/`. SQLite remains a rebuildable index; snapshot directories and manifests remain the source of truth.


### D12. Rust–TypeScript API typing — DECIDED

**Automatically generated TypeScript bindings from Rust contracts**

Rules:

- Rust command request/response types are the source of truth;
- TypeScript types and command wrappers are generated automatically;
- handwritten duplicate interfaces for native command contracts should be avoided;
- generated bindings are checked into the repository or regenerated deterministically in CI;
- the chosen generator must support Tauri 2 and remain replaceable behind the `src/lib/api/` boundary.
- before adoption, the generator is validated in a small proof of concept and pinned to an exact version;
- if no sufficiently reliable generator is available, the first vertical slice may use a narrow handwritten wrapper temporarily rather than block core backup work.

This keeps the native API auditable from TypeScript while preventing contract drift between Rust and the frontend.

### D13. UI component strategy — DECIDED

**Headless UI primitives + a custom lightweight design system**

Rules:

- use headless components for behavior-heavy, accessibility-sensitive controls such as dialogs, menus, tooltips, tabs and popovers;
- build the application-specific visual layer in Svelte rather than adopting a full opinionated UI framework;
- avoid committing to a detailed visual language before a dedicated design workshop;
- after the remaining architectural decisions are settled, run a separate, thorough design phase covering target aesthetic, information hierarchy, navigation, component language, density, states, accessibility and desktop-specific interaction patterns;
- implementation of polished UI components should not begin until that design direction is agreed.

### Planned design workshop

Before visual implementation, explicitly decide:

- desired visual character and references;
- desktop window structure and primary navigation;
- slot list and snapshot-history presentation;
- confirmation and destructive-action flows;
- empty, loading, error and recovery states;
- typography, spacing, density and iconography;
- light/dark mode expectations;
- accessibility and keyboard interaction requirements.


### D14. Frontend state management — DECIDED

**Local Svelte state by default; shared stores only for genuinely cross-screen state; no external state-management library.**

Rules:

- use Svelte 5 local reactive state for component and feature-local concerns;
- introduce a shared Svelte store only when the same state must be coordinated across multiple screens or long-lived UI regions;
- likely shared candidates include the active save directory, selected slot, application settings and refresh status;
- server/native data returned from Tauri commands should not be copied into a global store without a concrete need;
- do not add Redux-like or other external state-management libraries for the initial application.


### D15. Error contract across Rust and TypeScript — DECIDED

**Structured errors with a stable code, human-readable message and optional context.**

Rules:

- frontend behavior must branch on stable error codes rather than parsing message text;
- messages remain suitable for direct display or logging;
- context may contain non-sensitive diagnostic details such as paths, snapshot IDs or failed operation names;
- the initial taxonomy should stay small and expand only when the UI needs distinct handling.

### D16. Application updates — DECIDED

**Check for updates and notify the user, but do not install updates automatically in the first release.**

Rules:

- the application may check whether a newer stable release exists;
- when an update is available, show its version and provide an action opening the release/download page;
- no background download or automatic installation in the first release;
- update checks must not block startup or local backup/restore workflows;
- full in-app auto-update may be reconsidered after packaging and signing are stable.

### D5. GitHub backup implementation

To decide later: GitHub App/device flow, user-provided fine-grained PAT, and whether remote storage contains raw bundles, decoded representations, or both.

### D6. MVP scope — DECIDED

The first MVP is a safe local backup manager without decoding or editing the main save payload.

Included:

- automatic detection of Sailwind save directories;
- manual save directory selection;
- slot and full bundle discovery;
- slot list with thumbnail and metadata where available;
- manual snapshots;
- automatic safety snapshot before restore;
- snapshot history;
- labels and notes;
- restore;
- snapshot deletion;
- retention policy;
- checksum verification;
- opening save and backup directories;
- import and export of individual snapshots;
- recovery from interrupted operations and clear error handling.

Excluded from the first MVP:

- decoding or editing the main `.save` payload;
- semantic diffs;
- GitHub backup and synchronization;
- automatic backups triggered while the game is running;
- content deduplication.


### D9. Repository structure — DECIDED

**Hybrid single-application layout**

```text
/
├── src/
│   └── lib/
│       ├── api/
│       ├── components/
│       ├── features/
│       └── stores/
└── src-tauri/
    ├── src/
    │   ├── commands/
    │   ├── saves/
    │   ├── snapshots/
    │   ├── storage/
    │   └── platform/
    └── migrations/
```

The project remains a single Tauri application and build, while frontend and Rust code are organized by responsibility. Separate packages or Rust crates should be introduced only when a concrete reuse or compilation-boundary need appears.

### D17. Windows distribution — DECIDED

Initial distribution:

- GitHub Releases with a standard Windows installer;
- GitHub Releases with a portable ZIP build;
- builds are unsigned initially.

Planned later distribution:

- Microsoft Store using an MSIX package;
- Store publication is deferred until after the initial GitHub-distributed releases;
- Microsoft Store will be the preferred signed channel once introduced.

A separately purchased code-signing certificate is not required for the initial release.

### D18. Code signing — DECIDED

- Initial GitHub Releases will be unsigned.
- SmartScreen warnings are accepted for the initial release stage.
- A paid code-signing certificate will not be purchased initially.
- Microsoft Store signing will be used later for the Store-distributed MSIX package.

### D19. CI/CD and release publishing — DECIDED

**Fully automated GitHub Releases triggered by version tags**

Rules:

- pushing a version tag such as `v0.1.0` starts the release workflow;
- the workflow runs formatting checks, linting and all automated tests;
- release artifacts are published only when all required checks pass;
- GitHub Actions builds the Windows installer and portable ZIP;
- the workflow creates or updates the matching GitHub Release and uploads both artifacts;
- prerelease tags may publish prerelease builds without affecting the stable channel;
- ordinary branch pushes and pull requests never publish releases.


### D20. Release channels and versioning — DECIDED

Three automated release channels will be used:

- **Pull request snapshot:** every pull request build produces downloadable test artifacts. These are temporary CI artifacts, not GitHub Releases, and are clearly marked as untrusted development builds.
- **Nightly:** every merge to the default branch (`master` unless renamed later) publishes or refreshes a nightly prerelease with installer and portable ZIP artifacts. Nightly builds are never treated as stable.
- **Stable:** pushing a semantic version tag such as `v0.1.0` runs the full release pipeline and publishes a stable GitHub Release.

Release safeguards:

- pull request workflows cannot publish stable or nightly releases;
- nightly and stable publication requires all formatting, linting and tests to pass;
- artifacts include the commit SHA and application version in their metadata;
- stable version numbers follow Semantic Versioning;
- prerelease/nightly builds remain visibly distinguishable in the application UI and filenames.


### D21. Diagnostics and telemetry — DECIDED

The first release will not include telemetry or automatic crash reporting.

Diagnostic support will be user-initiated:

- the application keeps local logs with sensitive paths and save contents excluded or redacted where practical;
- the user can explicitly export a diagnostic ZIP;
- the report may include application version, operating-system information, recent structured errors, relevant logs and a manifest describing included files;
- exporting never sends data automatically;
- the application shows the exact report contents before the user shares it;
- save files and snapshot payloads are excluded by default and require a separate explicit action if ever needed for support.

### D22. Log format — DECIDED

Application logs will use **JSON Lines (JSONL)** as the canonical on-disk format.

Rules:

- one structured event per line;
- each event includes timestamp, severity, event or error code, component and optional structured context;
- sensitive paths and user data are redacted where practical;
- save contents and snapshot payloads are never logged;
- the application UI renders JSONL records in a readable human-oriented view;
- diagnostic exports include only the relevant recent log window rather than the complete history by default.


### D23. Application data location — DECIDED

All application data will be stored in the standard per-user application data directory. This applies to both the installed and portable builds. The portable ZIP changes distribution format only; it does not use application-adjacent storage.


### D24. Backup encryption — DECIDED

No encryption will be added for local snapshots or exported `.swbackup` files. Save files are not treated as sensitive data, and encryption would add unnecessary complexity to backup, restore and recovery workflows.

### D25. Automatic save-change detection — DECIDED

The MVP will not include a filesystem watcher or automatic snapshots triggered by game saves.

Rules:

- snapshots are created manually and automatically before restore operations;
- the application does not monitor the Sailwind save directory in the background;
- file-watcher behavior is explicitly deferred for later design and validation against Sailwind's actual save-write behavior;
- future work must define debounce, partial-write detection, duplicate-event handling, game-running behavior and the UX for automatic snapshots before implementation.

### D26. Restore safety policy — DECIDED

Every restore operation first creates an immutable safety snapshot of the current slot bundle. Only after that snapshot is complete and verified may the selected snapshot overwrite the live slot bundle.

### D27. Corrupted or incomplete snapshot restore — DECIDED, REVISED

Restore is always blocked when a snapshot is incomplete, contains missing files or fails checksum verification. The MVP has no force-restore override. The user may inspect validation failures and export recoverable contents for manual recovery without allowing the application to overwrite the live slot with a snapshot it cannot verify.

### D28. Restore while Sailwind is running — DECIDED

The application must detect whether the Sailwind process is running before starting a restore. Restore is blocked while the game is active. The user must close Sailwind and retry; there is no force-restore override for this condition in the MVP.


### D29. Game process detection — DECIDED

The application will detect a running Sailwind instance by process name only (for example `Sailwind.exe`).

This deliberately favors a simple and auditable implementation for the first release. The detection rule may be revisited if process renaming or false positives become a practical issue.

### D30. Active save directory — DECIDED

The application stores and operates on one active save directory at a time. Changing the directory replaces the previous active location rather than maintaining a list of known locations.


### D35. Application theme and visual direction — DECIDED

- Dark mode only for the first release.
- The visual language should be inspired by Sailwind without copying game assets or reproducing the game UI literally.
- Detailed visual design will be handled in a dedicated design workshop after the remaining architecture/product decisions are closed.
- Research direction: warm lantern-lit dark surfaces, aged wood and parchment references, restrained nautical instrumentation, soft low-contrast lighting, and sparse utilitarian controls.
- Avoid generic neon/gaming-dashboard styling, glossy sci-fi surfaces, and overdecorated skeuomorphism.


### D36. Navigation model — DEFERRED

The navigation model will not be fixed before the first UI prototype.

- prototype at least two plausible layouts, including master-detail and sidebar-based variants;
- evaluate them using the real MVP data and workflows rather than abstract wireframes alone;
- make the final choice during the planned design workshop;
- no architecture should make either option unnecessarily difficult before that decision.

## Decision log

- **2026-07-21 — D37 accepted:** Filesystem-changing workflows execute as complete Rust transactions with locking, staging, journaling and recovery semantics.
- **2026-07-21 — D38 accepted:** Editable annotations are durable versioned JSON files; SQLite remains fully rebuildable.
- **2026-07-21 — D39 accepted:** Restore reconciles exactly one recognized slot bundle and supports an initially absent target slot.
- **2026-07-21 — D40 accepted:** Imported archives and thumbnails are untrusted and subject to traversal, type and resource limits.
- **2026-07-21 — D41 accepted:** Bundle operations never follow symbolic links or junction-like redirects.
- **2026-07-21 — D42 accepted:** External game-process detection is implemented directly in Rust without shell commands.
- **2026-07-21 — D27 revised:** Invalid snapshots cannot be force-restored in the MVP.
- **2026-07-21 — D10 revised:** TypeScript owns presentation decisions; Rust enforces complete modifying workflows.
- **2026-07-21 — D4/D7 revised:** Durable annotations resolve rebuildability, and 50 snapshots is a configurable retention threshold rather than a hard cap.

- **2026-07-20 — D36 deferred:** Choose the navigation model after evaluating the first UI prototype during the dedicated design workshop.

- **2026-07-20 — D35 accepted:** Dark mode only; detailed UI design will be based on researched Sailwind visual language and finalized in a dedicated design workshop.

- **2026-07-20 — D30 accepted:** Keep exactly one active save directory; selecting another replaces it.

- **2026-07-20 — D29 accepted:** Detect a running Sailwind instance by process name only.

- **2026-07-20 — D28 accepted:** Restore is blocked while the Sailwind process is running; the user must close the game before retrying.

- **2026-07-20 — D27 accepted:** Invalid snapshots are blocked by default, with an explicit advanced force-restore path after detailed warnings.
- **2026-07-20 — D26 accepted:** Every restore first creates and verifies a safety snapshot of the current slot bundle.

- **2026-07-20 — D25 accepted:** No file watcher in the MVP; automatic change detection and snapshot behavior are deferred for a dedicated future decision.

- **2026-07-20 — D24 accepted:** Backups and exports will not be encrypted.

- **2026-07-20 — D23 accepted:** All application data uses the standard per-user application data directory, including the portable build.

- **2026-07-20 — D22 accepted:** JSON Lines is the canonical log format, rendered in a readable form in the UI and included selectively in diagnostic exports.


- **2026-07-20 — D21 accepted:** No telemetry or automatic crash reporting; users may explicitly export a reviewable diagnostic ZIP with local logs and environment metadata.


- **2026-07-20 — D20 accepted:** PR snapshot artifacts, nightly prereleases after every merge to `master`, and stable GitHub Releases on version tags.

- **2026-07-20 — D19 accepted:** Fully automated GitHub Releases triggered by version tags, gated by formatting, linting and automated tests.

- **2026-07-20 — D34 accepted:** English-only initial interface with an i18n-ready translation-key architecture.
- **2026-07-20 — D33 accepted:** UTC timestamps in persisted data and logs; local time in the UI.
- **2026-07-20 — D32 accepted:** Versioned JSON file for application settings.

- **2026-07-20 — D18 accepted:** No paid code-signing certificate for initial releases; Store signing planned later for MSIX distribution.

- **2026-07-20 — D17 accepted:** Initial distribution through unsigned GitHub Releases (installer + portable ZIP); Microsoft Store deferred until later.

- **2026-07-20 — D1 accepted:** Tauri 2 + TypeScript frontend + Rust native core.
- Native and safety-critical filesystem logic will live in Rust; application UI and presentation logic will live in TypeScript.
- **2026-07-20 — D2 accepted:** Svelte 5 + Vite + TypeScript strict, with Vitest and Playwright.
- SvelteKit, a global state library and a dedicated router are excluded from the initial stack.
- **2026-07-20 — D3 accepted:** Windows is officially supported for the first release; Linux and Steam Deck are experimental; macOS is out of scope.
- **2026-07-20 — D4 accepted:** Immutable snapshot directories with SQLite as a rebuildable metadata index.
- **2026-07-20 — D6 accepted:** The first MVP is a local backup manager without save decoding or editing.
- **2026-07-20 — D3 accepted:** Windows is officially supported for the first release; Linux/Steam Deck support is experimental; macOS is out of scope.
- **2026-07-20 — D4 accepted:** immutable full-bundle snapshot directories with `manifest.json`, plus SQLite as a rebuildable metadata index.
- **2026-07-20 — D3 accepted:** Windows is officially supported for the first release; Linux / Steam Deck support is experimental; macOS is out of scope.
- **2026-07-20 — D7 accepted:** ULID-based immutable snapshots with per-file SHA-256 checksums, `.swbackup` ZIP export and a limit of 50 snapshots per slot with a user warning before automatic deletion.
- **2026-07-20 — D8 accepted:** The first vertical slice covers the complete local backup flow from save discovery through snapshot history and safe restore.
- **2026-07-20 — D9 accepted:** Hybrid single-application repository structure with logical frontend and Rust modules; no monorepo or extracted crates initially.
- **2026-07-20 — D10 accepted:** Thin Rust core for native and persistence primitives, with application workflow and user-facing rules in TypeScript.
- **2026-07-20 — D11 accepted:** `rusqlite` with explicit, versioned SQL migrations.
- **2026-07-20 — D12 accepted:** TypeScript bindings and command wrappers are generated automatically from Rust contracts.
- **2026-07-20 — D10 accepted:** Thin Rust core for native and persistence primitives; application workflow and user-facing rules remain in TypeScript.
- **2026-07-20 — D11 accepted:** `rusqlite` with explicit versioned SQL migrations; SQLite is a rebuildable index, not the source of truth.
- **2026-07-20 — D13 accepted:** Headless UI primitives with a custom lightweight design system; a dedicated design workshop is required after the remaining architectural decisions and before polished UI implementation.
- **2026-07-20 — D14 accepted:** Local Svelte state by default, shared Svelte stores only for genuinely cross-screen state, and no external state-management library.

- **2026-07-20 — D15 accepted:** Structured cross-boundary errors with a stable code, readable message and optional context.
- **2026-07-20 — D16 accepted:** The first release checks for updates and notifies the user with a link to the release page, without automatic installation.

### D31. Backup directory configuration — DECIDED

**Fixed application-data directory**

Rules:

- snapshots, manifests and the SQLite index are stored under the standard per-user application data directory;
- the backup location is not user-configurable in the first release;
- the portable build uses the same application-data location rather than storing backups next to the executable;
- the UI should provide an action to open the backup directory in the system file manager.


### D32. Application settings storage — DECIDED

**Versioned JSON settings file**

Rules:

- application preferences are stored in a small, human-readable JSON file;
- the file contains a `schemaVersion` field and is migrated explicitly when the schema changes;
- settings include the active save directory, update-check preference, theme and future UI preferences;
- critical snapshot metadata does not live in the settings file.

### D33. Date and time handling — DECIDED

**UTC in persisted data, local time in the UI**

Rules:

- manifests, SQLite metadata and logs store timestamps in UTC using an unambiguous ISO 8601 representation;
- the UI displays timestamps in the user's local timezone;
- diagnostic views may additionally expose the original UTC value.

### D34. Interface language and localization — DECIDED

**English-only initial release with i18n-ready architecture**

Rules:

- the first release ships with an English interface;
- user-facing strings are referenced through translation keys rather than embedded directly in components;
- Polish and other languages may be added later without restructuring the UI;
- diagnostic codes, manifest fields and machine-readable values remain language-neutral.

### D37. Filesystem transaction and recovery model — DECIDED

- “Atomic” bundle operations mean crash-recoverable application transactions; the application does not claim that multiple independent filesystem entries can be replaced by one operating-system atomic rename.
- every modifying operation uses an exclusive application-level lock, a staging directory and a durable operation journal;
- restore verifies the source and the newly written destination;
- interrupted operations are detected and recovered or safely reported at the next startup;
- SQLite transactions protect the index only and are never treated as a transaction covering live save files.

### D38. Snapshot annotations — DECIDED

```text
app-data/
  snapshots/<slot>/<ulid>/
    manifest.json
    bundle/
  annotations/<ulid>.json
  index.db
```

- `manifest.json` and `bundle/` are immutable;
- `annotations/<ulid>.json` is the durable source of truth for editable label, note and protection state;
- annotation files use versioned JSON and atomic replacement;
- SQLite can be rebuilt from manifests and annotation files without losing user-created metadata or retention protection.

### D39. Restore set semantics — DECIDED

- restore reconciles only the recognized artifacts belonging to the selected slot;
- artifacts currently present for that slot but absent from the selected snapshot are removed as part of the transaction, after the current bundle has been captured in a verified safety snapshot;
- other slots and unrelated files in the save directory are never modified;
- when the target slot is absent, the journal records that initial state and restore proceeds without an empty safety snapshot.

### D40. Untrusted file and archive policy — DECIDED

- imported `.swbackup` archives and their thumbnails are treated as untrusted input;
- archive extraction rejects absolute paths, parent traversal, symlinks, duplicate paths, unsupported entry types and manifest mismatches;
- import enforces limits on entry count, compressed size, expanded size and thumbnail dimensions;
- an imported snapshot cannot silently replace an existing snapshot with the same ULID;
- snapshot thumbnails are decoded with explicit size limits before display when they originate from an imported archive.

### D41. Symbolic link policy — DECIDED

- bundle discovery and copying do not follow symbolic links or junction-like redirects;
- encountering one produces a structured warning or error identifying the affected relative path;
- a sidecar directory cannot cause data outside the active save directory to be included in a snapshot.

### D42. Game process detection implementation — DECIDED

- Sailwind process discovery is implemented in Rust using a direct system/process enumeration library;
- the application does not invoke a shell command such as `tasklist` for process detection;
- Tauri's process plugin is not used for discovering external processes because it manages the application process itself.
