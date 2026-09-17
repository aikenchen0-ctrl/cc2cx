# cc-launch 3.20.6 Windows artifacts

These are the release-mode Windows installers built from commit
`405f844a50182f0603b2fcb8fb7d161309aba350`.

| Artifact | Format | Size | SHA-256 |
| --- | --- | ---: | --- |
| `cc-launch_3.20.6_x64-setup.exe` | NSIS per-user installer | 11,859,464 bytes | `CC834583A732FBF3D7ABCB2DAD5A9DB6D6C834650D14514932AAD025A08EC038` |
| `cc-launch_3.20.6_x64_en-US.msi` | Windows Installer | 17,534,976 bytes | `8281FFCB161E29BFB2C7109A9CE5B35D799E6A9965D6C37CB3040AF227AE9EA7` |

The installers are currently unsigned. Verify the SHA-256 value before
installing. Do not distribute `src-tauri/target/debug/cc-launch.exe`; that is
a development launcher and is not a portable release artifact.
