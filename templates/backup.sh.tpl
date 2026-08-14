#!/usr/bin/env bash
# {{PROJECT}} — backup script (edit me for custom DB dumps etc.)
set -euo pipefail

STAMP="$(date +%Y%m%d-%H%M%S)"
BACKUP_DIR="{{BACKUP_DIR}}"
TMP="$BACKUP_DIR/.tmp-$STAMP"
mkdir -p "$TMP"

{{SECTIONS}}

ARCHIVE="$BACKUP_DIR/backup-$STAMP.tar.gz"
tar -czf "$ARCHIVE" -C "$TMP" .
rm -rf "$TMP"

# retention: keep the newest {{RETENTION}} backups
cd "$BACKUP_DIR"
ls -1t backup-*.tar.gz 2>/dev/null | tail -n +{{RETENTION_PLUS_1}} | xargs -r rm -f --

echo "✅ backup saved: $ARCHIVE"