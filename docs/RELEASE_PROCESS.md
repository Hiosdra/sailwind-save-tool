# Release process

The alpha pipeline is intentionally review-gated. It creates an unsigned draft prerelease and never publishes it automatically.

## Prepare

1. Merge a release-preparation pull request into `master` after all required CI jobs pass.
2. Ensure `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` contain the same version.
3. Move the matching changelog section out of `Unreleased` and add the release date.
4. Complete the automated and relevant manual checks in [ALPHA_TEST_CHECKLIST.md](./ALPHA_TEST_CHECKLIST.md).

## Build a draft

1. Open **Actions → Draft alpha release → Run workflow**.
2. Select `master` and enter the exact prerelease tag represented by the application version, for example `v0.1.0-1`.
3. The workflow validates the tag/version pair, runs frontend and Rust checks, then builds unsigned x64 NSIS, MSI and portable ZIP artifacts.
4. The workflow creates a draft GitHub prerelease. It does not publish the release.

## Review and publish

1. Download every artifact from the draft and test it on Windows.
2. Confirm installer and portable filenames, version metadata and startup behavior.
3. Review generated release notes; call out unsigned binaries and alpha status prominently.
4. Record test results and link any blocking issues.
5. Publish manually only when the checklist passes. Delete a rejected draft rather than reusing its artifacts after code changes.

## Recovery from a failed workflow

No release is published when validation or packaging fails. Fix the cause through a new pull request. If a draft was created before a later upload step failed, delete that incomplete draft before rerunning the workflow.

Stable and nightly channels remain roadmap work; this alpha workflow must not be repurposed to silently publish either channel.
