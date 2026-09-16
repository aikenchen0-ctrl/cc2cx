# cc-launch 3.20.5 Windows artifacts

These are the release-mode Windows installers built from commit
`1d508e983fc136a2ee3c3d0334a98d42a1f35459`.

| Artifact | Format | Size | SHA-256 |
| --- | --- | ---: | --- |
| `cc-launch_3.20.5_x64-setup.exe` | NSIS per-user installer | 11,866,272 bytes | `4D64E3A1619F6051C66368018A8C68F6AFB58EE7D97559553582140A0D539622` |
| `cc-launch_3.20.5_x64_en-US.msi` | Windows Installer | 15,904,768 bytes | `6F166DB0F49357ACF3386A29849EA8A38D4BBA1973A6B8B9413AACD6758AB7BF` |

The installers are currently unsigned. Verify the SHA-256 value before
installing. Do not distribute `src-tauri/target/debug/cc-launch.exe`; that is
a development launcher and is not a portable release artifact.
