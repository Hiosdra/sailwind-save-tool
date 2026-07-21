# Alpha test checklist

Use disposable copies of real saves for the first pass. Keep an independent copy outside the Sailwind directory before testing restore behavior.

Record the application version, Windows version and artifact type (NSIS, MSI or portable ZIP) with every result.

## Artifact provenance

- [ ] Download the artifact from a successful workflow run or draft release in this repository.
- [ ] Confirm the expected files are present and no browser/antivirus product reports a known threat.
- [ ] Note the expected unsigned-publisher SmartScreen warning, if shown.

## Installation and startup

- [ ] Install NSIS for the current user and launch the app.
- [ ] Install MSI and launch the app on a clean test account or VM.
- [ ] Launch the portable ZIP build without installing it.
- [ ] Upgrade from the previous alpha and confirm snapshots/settings remain available.
- [ ] Uninstall and confirm the game save directory is untouched.

## Discovery and snapshots

- [ ] Detect the default Windows Sailwind save directory.
- [ ] Select a directory manually.
- [ ] Discover each occupied slot and all expected metadata, thumbnails, game backups and mod sidecars.
- [ ] Reject an incomplete slot with no primary `.save` file.
- [ ] Create a snapshot and confirm its file count and total size are plausible.
- [ ] Add a label, protect the snapshot and confirm deletion is blocked.

## Restore safety

- [ ] Close Sailwind, modify a disposable save bundle and restore a known snapshot.
- [ ] Confirm a safety snapshot is created before the live bundle changes.
- [ ] Confirm files absent from the selected snapshot are removed from the restored bundle.
- [ ] Launch Sailwind and confirm restore is blocked while `Sailwind.exe` is running.
- [ ] Force-close the manager during restore in a disposable directory, reopen it and verify the recovery notice and resulting bundle.
- [ ] Confirm a checksum-corrupted snapshot cannot alter the live save.

## Import and export

- [ ] Export a labeled/protected snapshot to `.swbackup`.
- [ ] Import it into a clean application-data directory and compare file counts and checksums.
- [ ] Confirm importing the same snapshot twice is rejected.
- [ ] Confirm a malformed or truncated archive fails without creating a history entry.

## Diagnostics and privacy

- [ ] Open Diagnostics and confirm recent operations and outcomes are readable.
- [ ] Copy the report and confirm it contains no save path, Windows account name, labels, notes or save contents.
- [ ] Acknowledge a recovery notice and confirm it does not reappear after restart.

## Retention and cleanup

- [ ] Reach the 50-snapshot threshold in a disposable slot and confirm explicit approval is required.
- [ ] Confirm protected snapshots are never selected for pruning.
- [ ] Confirm no `.staging-*`, `.sailwind-restore-*` or `.sailwind-rollback-*` directories remain after successful operations.

## Result

- Tester:
- Date:
- Version/tag:
- Windows version:
- Artifact:
- Passed sections:
- Failed checks and linked issues:
