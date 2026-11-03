# TablesCraftLocal 0.9.6-local.20261103

Prepared November 3, 2026; unpublished. Proposed tag: `v0.9.6-local.20261103`.

Desktop 0.9.6; installer 0.9.7. The executable is unchanged from the tested frozen baseline. This revision completes the source/asset notice payloads in the MSI, Setup and portable ZIP. PDF also includes LICENSE-document.txt beside the executable. Original component/font notices are retained in full, with source-supplied license texts appended.

Baseline source: `bb82db7c00ff47a98b2b702b89c0b1c6bba9461f`. Executable SHA-256: `b7577f0a3da319806b30168127f0aa4cd3efff848d530574427d02bbcef2ac6c`. The source import adds release documentation and packaging, removes upstream marketing marks/automatic workflows, and reconciles stale PDF attribution hashes against the unchanged baseline files. Application code, Cargo manifests, lockfile and runtime assets are unchanged.

Later shared-formula work is excluded. Legacy GridCraft profile/workbook/association identifiers remain where needed for migration.

The local editions provide explicit Preview + Send Q&A with a separately configured local runtime. Model weights and LM Studio are not included. Builds are unsigned. No complete Office/Reader equivalence is claimed.

Package validation: every final MSI file is extracted and compared with the expected payload; Setup must embed that exact MSI; portable files must match the same payload plus the portable marker. Compiled registry/shortcut policy is checked against tested baseline authoring, with no custom actions, no UserChoice writes and no automatic shutdown. Installer lifecycle was not replayed on the workstation. Historical application/installer tests remain retained and correctly scoped. Final scan and checksum receipts belong to the complete suite manifest.

The installer package version advances in the first three fields to permit upgrade from the previous package. Application version strings remain 0.9.6. Keep copies of important documents/settings and close the app before updating. Portable Data is private and excluded.

After publication, use only version-specific GitHub release URLs and verify SHA256SUMS.txt. The suite website will use those same files. No repository/tag/release has been published by local preparation.
