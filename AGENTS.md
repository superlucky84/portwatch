# Project guidance

DDD means **Doc-Driven Development** in this project.

## Read first

1. docs/REQUIREMENTS.md
2. docs/DESIGN.md
3. docs/IMPLEMENT.md
4. docs/MANUAL_TEST_CHECKLIST.md

## Documentation workflow

Use the locally installed doc-driven-designer-v1 skill for requirements and design work.
Keep these four files as the canonical plan and update them before changing a contract.
Each implementation phase needs baseline validation and exit criteria; retain dedicated
Test Hardening and Integration Test phases. Keep confirmed requests, design proposals,
and implemented behavior distinct. Record user decisions in all affected documents.

Track unresolved decisions explicitly as TBD. Keep one current handoff block at the end of
docs/IMPLEMENT.md with completed work, next steps, blockers, and the latest known commit.
Phases carry `Refs:` lines, and `Files:` blocks once implemented. Treat the records as
search hints and also search the document text.

## Source and checks

Phase 0 created this layout. Do not describe commands as available before they exist.

- The root Cargo workspace holds crates/portwatch-core, crates/portwatch-collect, and src-tauri.
  Pin Tauri crates and the Tauri CLI to exact versions and record them in docs/DESIGN.md.
- crates/portwatch-core/ is pure Rust: no process spawning, file system, network, or macOS APIs.
  It takes command output text and returns values. Test it with synthetic fixtures.
- crates/portwatch-collect/ runs commands by absolute path with argument arrays (no shell) and LC_ALL=C.
- src-tauri/ exposes only fixed commands to the UI. Do not add a shell plugin or arbitrary command execution.
- ui/ renders snapshots only. Use JavaScript ES modules with two-space indentation and semicolons.
- Run npm run check (cargo fmt, clippy with warnings denied, document links and IDs) and npm test for
  every change. Run npm run check:app for src-tauri changes; on Linux it needs the WebKitGTK development packages.
  npm run test:ui arrives in Phase 2.
- macOS CI builds run only by manual dispatch because macOS minutes cost about ten times Linux minutes.
- Behavior that only macOS shows must be verified on the user's Mac or macOS CI; record which one.

## Safety

- Detection never uses sudo, and the app never runs as root.
- Use administrator privileges only for one confirmed stop command through the macOS password dialog.
  Build that command from a fixed template and validated values.
- Never offer to stop protected entries: PID 1, launchd sockets, system or security processes,
  portwatch itself, or other users' processes.
- Stop launchd-managed services through their manager instead of signalling the process.
- Re-check the PID, start time, and command line right before sending a signal.
- Read hosts, proxy, and plist files only; never modify them.
- Send no network traffic except loopback TLS inspection; add no telemetry.

## Fixtures and privacy

- Use synthetic, redistributable fixtures based on observed output formats.
- Never commit raw output from the user's Mac, user names, home paths, host names, or company details. Use `.test` domains.

## Git

- Work on `main` (user decision, 2026-11-18).
- Prefer locally installed skills.
