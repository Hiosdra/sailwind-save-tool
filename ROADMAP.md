# Roadmap

The roadmap is ordered by risk reduction and release readiness. Dates are intentionally omitted until the alpha has been exercised with real save bundles.

## Alpha foundation

- [x] Complete save-bundle discovery and immutable verified snapshots.
- [x] Safety snapshot, staged restore, rollback journal and startup recovery.
- [x] Hardened `.swbackup` import/export and retention protection.
- [x] Privacy-safe operation diagnostics and recovery notice.
- [x] Windows NSIS/MSI/portable artifacts and draft prerelease workflow.
- [ ] Complete the manual alpha checklist on Windows with disposable copies of real saves.

## Release candidate

- [ ] Add end-to-end desktop tests for the highest-risk workflows.
- [ ] Export a user-reviewable diagnostic ZIP with bounded logs.
- [ ] Add opt-in update checks that only open the GitHub release page.
- [ ] Validate Windows installer upgrades and uninstalls across alpha versions.
- [ ] Finalize release notes, screenshots and accessibility review.

## Later

- [ ] Evaluate code signing after real-world demand is understood.
- [ ] Expand experimental Steam Deck support and testing.
- [ ] Consider safe save decoding only after the raw backup manager is stable.

The detailed accepted decisions remain in [plan-16.md](./plan-16.md).
