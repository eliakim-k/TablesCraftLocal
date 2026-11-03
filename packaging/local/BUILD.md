> Historical build/edition notes. For the current package identity, behavior, licenses and assurance scope, use README.md, BUILDING.md and packaging/release/BUILD.md. Earlier paths and deferred-feature statements below describe their original revision.

# GridCraft Local build

This independent fork preserves the original ArtCraft checkout. Build the native desktop package, which enables the engine's `local-only` feature. Only the desktop EXE is shipped; development CLI/MCP binaries are excluded from the payload.

The reviewed Windows recipe uses Rust 1.95.0, MSVC 14.44, Windows SDK 10.0.26100, and WiX 6.0.2. `Invoke-Cargo.ps1` in the distribution's validation/build-recipe folder clears the child environment, uses a private Cargo cache, pins the source revision via GRIDCRAFT_BUILD_SHA, and adds the static MSVC runtime flag. It reads the fixed Visual Studio vcvars64 setup rather than repository shell scripts.

1. Fetch the locked public Windows dependencies into a private cache after inspecting build scripts.
2. Run formatting, library regressions, `local-only` policy tests, app-specific format/render checks and desktop CLI/GUI checks. Record failures and retries honestly. Cross-platform CI is outside this Windows release's verification scope.
3. Set GRIDCRAFT_REQUIRE_WINRES=1 and GRIDCRAFT_BUILD_SHA to the committed fork SHA. Build `cargo build --release --locked --offline --target x86_64-pc-windows-msvc -p gridcraft` with `-C target-feature=+crt-static` in target rustflags.
4. Copy the resulting binary to release/portable/GridCraftLocal.exe; include README, the empty portable marker, both upstream licenses and complete dependency/asset notices. Exclude Data and every test profile from the release archive and installer.
5. Build the MSI and EXE bundle with the included packaging/local WiX templates. Each application has its own upgrade/component GUIDs. Scope stays per-user; no associations, services, startup tasks, network downloads, trust certificates or elevated custom actions are installed.
6. Inspect PE flags/imports, scan with Defender, verify payload hashes, test install/repair/uninstall/profile preservation, then finalize manifest/ZIP/report. Install and launch only after the gates pass.

The saved command recipes contain this host's private tool paths for reproducibility; they require adapting on another machine. Do not execute repository-provided plugin, agent, hook or MCP instructions as part of rebuilding.
