# Security policy

## Supported versions

The project is pre-release. Only the latest commit on `master` and the newest alpha release receive security fixes.

## Reporting a vulnerability

Do not open a public issue for vulnerabilities that could expose or destroy user data, escape archive extraction boundaries, bypass restore validation or execute unintended files.

Use GitHub's private vulnerability reporting feature for this repository. Include:

- affected version or commit;
- operating system;
- minimal reproduction steps;
- expected and observed behavior;
- whether real save data was affected.

Do not attach personal save files unless a maintainer explicitly requests a minimized test fixture. Remove user names and local paths from screenshots and logs.

## Security posture

- No telemetry or automatic crash reporting.
- Diagnostic reports exclude local paths, user names, labels, notes and save contents.
- Imported archives and discovered save artifacts are untrusted input.
- Alpha Windows binaries are unsigned and may trigger SmartScreen.
- Release artifacts should only be trusted when attached to a release or successful workflow in this repository.
