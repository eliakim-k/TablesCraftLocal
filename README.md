# TablesCraftLocal

Work with spreadsheets. Windows x64 local edition, with optional question answering through a separately installed local model runtime.

Independent local adaptations by Eliakim M. Kambale, based on [gridcraft by the ArtCraft team and contributors](https://github.com/storytold/gridcraft). This is an independent fork; upstream endorsement is not claimed.

## Release 0.9.6-local.20261103

Desktop version: **0.9.6**. MSI/Setup version: **0.9.7**. This is a packaging and notice revision of the frozen November 2 executable, whose SHA-256 is `b7577f0a3da319806b30168127f0aa4cd3efff848d530574427d02bbcef2ac6c`. It adds complete source/asset notices while preserving the application payload. See [release notes](RELEASE_NOTES.md) and [source provenance](SOURCE_PROVENANCE.json).

The public source is a snapshot import with upstream credits preserved. Its application source, Cargo manifests, lockfile and runtime assets match the baseline commit `bb82db7c00ff47a98b2b702b89c0b1c6bba9461f`. Differences are release documentation/packaging and, for PDF, reconciled attribution metadata. ArtCraft marks and inherited automatic workflows are omitted. Historical Git bundles and private development workspaces are not included.

This local checkout and tag are prepared for publication. No remote repository or release has been created by this preparation. The intended destination is `eliakim-k/TablesCraftLocal`. After publication, the versioned release will provide a Setup EXE, MSI, portable ZIP, source ZIP, release notes and SHA256SUMS.txt. The suite website will link to the same immutable assets.

## Installation

Run the per-user Setup EXE, or use the MSI. The application payload requires x64 Windows. Retained startup checks were performed on Windows 11; a broader supported hardware/Windows matrix is not claimed. The build is **unsigned**. Keep Windows security protections enabled.

Save documents and close this app yourself before updating. The installer does not force-close apps or write default-file UserChoice selections. Use Windows Settings > Apps to remove an installed copy. Back up documents, settings and recovery separately. The installer version advances in the first three fields so this packaging revision can replace the earlier MSI package; the desktop version is unchanged.

For portable use, extract into a writable folder and keep `TablesCraftLocal.portable` beside `TablesCraftLocal.exe`. Settings/recovery use the adjacent private `Data` folder. Do not distribute that folder. Installed profiles use LocalAppData; TablesCraft intentionally retains legacy GridCraft identities where needed for migration. Developer control/MCP/CLI binaries, model weights and LM Studio are not bundled.

## Optional local AI

See [the connection and context guide](docs/LOCAL-AI.md). Configure a separately installed compatible model runtime on the literal loopback address `127.0.0.1`, choose a loaded model in the Settings gear, inspect the question and excerpts with **Preview**, then explicitly press **Send to local AI**. Answers can be read or copied and do not automatically edit documents. Selected excerpts may omit relevant content. A live model was not contacted during packaging validation.

## Changes and verification

The local fork adds explicit preview/send Q&A, selection context actions, gear connection settings, richer contextual editing/paste and formatted answers. Word includes its recorded return-to-editor PDF-export fix. TablesCraft includes its visible rename while preserving migration identities. The original application remains credited to its upstream authors.

Later shared-formula work is excluded. Legacy GridCraft profile/workbook/association identifiers remain where needed for migration. Full Microsoft Office or Adobe Reader equivalence is not claimed. Test important files on copies.

Final package verification extracts the MSI and Setup, compares every payload file with the portable package, and checks the compiled per-user registry/shortcut policy against the previously tested installer authoring. Executable hashes remain unchanged. The final wrappers were not installed on this workstation; historical lifecycle/startup checks remain scoped to the baseline application and original policy. Source asset-policy checks are static Python executions of the reviewed xtask predicates, not a new Rust CI run. Historical Word baseline CI is green; newer development CI/search limitations do not apply as shipped features here. Exact verification/scan receipts are retained in the suite release evidence.

## Building and licensing

See [BUILDING.md](BUILDING.md) and [the release packaging recipe](packaging/release/BUILD.md). The retained application build used Rust 1.95.0, MSVC 14.44 and the x64 Windows target with a static runtime. Packaging uses pinned WiX 6.0.2. No independent bit-for-bit rebuild is claimed.

Software is offered under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. Preserve [NOTICE](NOTICE), [asset attribution](ATTRIBUTION.md), and all component/font terms. New maintainer source documentation uses the same dual license. ArtCraft marks have separate terms and are omitted from the fork. The Contributions Ledger article CC BY-NC policy does not replace software licenses.
