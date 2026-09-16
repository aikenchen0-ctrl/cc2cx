# CASR vendor snapshot

This directory is the build-time copy of CASR used by cc2cx. The upstream
checkout is kept at `../../casr` for review and synchronization; fixes should
be made there first, tested, and then copied into this directory.

The snapshot was based on upstream commit `285aa11` and includes the cc2cx
session-transfer extensions plus the Codex thread-index rollback fix. Build
artifacts and the upstream `.git` directory are intentionally excluded.
