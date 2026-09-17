# cc-launch 3.20.7 Windows artifacts

These are the release-mode Windows installers built from commit
`8a3aa58`.

| Artifact | Format | Size | SHA-256 |
| --- | --- | ---: | --- |
| `cc-launch_3.20.7_x64-setup.exe` | NSIS per-user installer | 11,866,348 bytes | `D75DD27B2262AA9BB7B1303E4311B552CA7C6E9ACD3574FA451CD4B37438B015` |
| `cc-launch_3.20.7_x64_en-US.msi` | Windows Installer | 17,534,976 bytes | `EED65C696A04AC0EB2B31BF4C97280A8AB874FE890A27CFC56A961BF29A4274B` |

The installers are currently unsigned. Verify the SHA-256 value before
installing. Do not distribute `src-tauri/target/debug/cc-launch.exe`; that is
a development launcher and is not a portable release artifact.
