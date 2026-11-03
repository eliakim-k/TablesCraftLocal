# Notice-only packaging recipe

Use the supplied TablesCraftLocal.wxs and Bundle.wxs with WiX 6.0.2 and WixToolset.BootstrapperApplications.wixext 6.0.2. These tools are build dependencies, not application payloads. The retained receipt records the exact NuGet SHA-512 checks. The local preparation ran the .NET 6-targeted WiX DLL through an existing .NET 8 runtime with DOTNET_ROLL_FORWARD=Major; it did not install system software.

Create a payload directory containing the verified TablesCraftLocal.exe (SHA-256 `b7577f0a3da319806b30168127f0aa4cd3efff848d530574427d02bbcef2ac6c`) and every file in payload-docs/. For portable distribution add an empty TablesCraftLocal.portable marker; omit it from the installed payload. Do not copy private Data/settings/recovery. SOURCE_PROVENANCE.json records exact wrapper versions and output hashes.

With WIX_DLL, BAL_EXTENSION, SOURCE_DIR, PAYLOAD_DIR and OUTPUT_DIR assigned to reviewed paths, the command forms are:

```powershell
dotnet $WIX_DLL build "$SOURCE_DIR/packaging/release/TablesCraftLocal.wxs" -arch x64 -d Version=0.9.7 -d "SourceDir=$SOURCE_DIR" -d "PayloadDir=$PAYLOAD_DIR" -o "$OUTPUT_DIR/TablesCraftLocal.msi"
dotnet $WIX_DLL build "$SOURCE_DIR/packaging/release/Bundle.wxs" -arch x64 -ext $BAL_EXTENSION -d Version=0.9.7.0 -d "SourceDir=$SOURCE_DIR" -d "MsiPath=$OUTPUT_DIR/TablesCraftLocal.msi" -d "LicenseRtf=$SOURCE_DIR/packaging/local/license.rtf" -o "$OUTPUT_DIR/TablesCraftLocal-Setup.exe"
```

Use private intermediate/TEMP directories and record commands, source tag, package hashes and resulting artifacts. WiX assigns new product/package identifiers, so binary-identical MSI rebuilds are not claimed. MajorUpgrade retains the original upgrade lineage. Changing only a fourth MSI version field is insufficient for upgrade comparison; this revision advances the third field.

Before distribution verify extracted MSI files, the exact MSI embedded in Setup, all portable files, notice coverage, registry/shortcut policy, versions and architecture; retain scans and checksums. The local candidate did not replay installation lifecycle or modify installed apps.
