#!/bin/sh
# Restore a backup made by backup/backup.sh. Run on the server in deploy/.
#
#   ./restore.sh <age-private-key-file> <timestamp>
#   ./restore.sh ~/backup-key.txt 2026-10-16_0300
#
# Needs `age` on the host (apt install age). OVERWRITES the current database
# and photo store. The private key file should only be on the server for the
# duration of the restore.
set -eu

KEY="${1:?usage: restore.sh <age-private-key-file> <timestamp>}"
TS="${2:?usage: restore.sh <age-private-key-file> <timestamp>}"
DB="backups/plants-db-$TS.dump.age"
PHOTOS="backups/plants-photos-$TS.tar.age"
[ -f "$DB" ] || { echo "missing $DB" >&2; exit 1; }
[ -f "$PHOTOS" ] || { echo "missing $PHOTOS" >&2; exit 1; }

printf 'This replaces the current database and photos with the backup from %s. Type RESTORE to continue: ' "$TS"
read -r answer
[ "$answer" = "RESTORE" ] || { echo "aborted"; exit 1; }

echo "stopping backend"
docker compose stop backend

echo "restoring database"
age --decrypt --identity "$KEY" "$DB" \
	| docker compose exec -T db pg_restore --clean --if-exists --no-owner -U plants -d plants

echo "restoring photos"
age --decrypt --identity "$KEY" "$PHOTOS" \
	| docker compose run --rm -T --no-deps --entrypoint sh -v plants_photos:/restore backup \
		-c 'find /restore -mindepth 1 -delete && tar -C /restore --strip-components=1 -xf -'

echo "starting backend"
docker compose start backend
echo "done"
