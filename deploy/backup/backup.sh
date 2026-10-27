#!/bin/sh
# Nightly encrypted backup of the database and the photo store.
#
#   backup.sh daemon   run once a day at BACKUP_HOUR (default, container CMD)
#   backup.sh now      run one backup immediately
#
# Both parts are streamed straight into `age` encryption: no unencrypted copy
# is ever written to disk. Only the public key (BACKUP_AGE_RECIPIENT) is on the
# server; restoring needs the private key, which belongs offline.
# Files older than BACKUP_KEEP_DAYS (default 30) are deleted, matching the
# privacy policy.
set -eu
# A failing pg_dump must fail the pipeline, not hide behind age's success.
set -o pipefail

: "${BACKUP_AGE_RECIPIENT:?BACKUP_AGE_RECIPIENT (age public key) is required}"
KEEP_DAYS="${BACKUP_KEEP_DAYS:-30}"
HOUR="${BACKUP_HOUR:-3}"
DIR=/backups

log() { echo "$(date '+%Y-%m-%d %H:%M:%S') $*"; }

# Explicit `|| return 1` throughout: `set -e` does not apply inside `if run_backup`.
run_backup() {
	ts=$(date +%Y-%m-%d_%H%M)
	db_file="$DIR/sowling-db-$ts.dump.age"
	photo_file="$DIR/sowling-photos-$ts.tar.age"

	log "backup $ts: database"
	if ! pg_dump --format=custom --no-owner --no-privileges \
		| age --encrypt --recipient "$BACKUP_AGE_RECIPIENT" --output "$db_file.part"; then
		rm -f "$db_file.part"
		log "database dump FAILED"
		return 1
	fi
	mv "$db_file.part" "$db_file" || return 1

	log "backup $ts: photos"
	if ! tar -C /data -cf - photos \
		| age --encrypt --recipient "$BACKUP_AGE_RECIPIENT" --output "$photo_file.part"; then
		rm -f "$photo_file.part"
		log "photo archive FAILED"
		return 1
	fi
	mv "$photo_file.part" "$photo_file" || return 1

	find "$DIR" -name 'sowling-*.age' -type f -mtime +"$KEEP_DAYS" -print -delete | sed 's/^/deleted old backup: /'
	find "$DIR" -name '*.part' -type f -mmin +120 -delete
	log "backup $ts done: $(du -h "$db_file" | cut -f1) database, $(du -h "$photo_file" | cut -f1) photos"
}

case "${1:-daemon}" in
	now)
		run_backup
		;;
	daemon)
		log "backup daemon: daily at ${HOUR}:00 (${TZ:-UTC}), keeping ${KEEP_DAYS} days"
		last=""
		while true; do
			today=$(date +%Y-%m-%d)
			# "03" -> "3", "00" -> "0", to compare with BACKUP_HOUR.
			hour=$(date +%H | sed 's/^0\(.\)/\1/')
			if [ "$hour" = "$HOUR" ] && [ "$last" != "$today" ]; then
				if run_backup; then last="$today"; else log "backup FAILED, retrying in 10 minutes"; fi
			fi
			sleep 600
		done
		;;
	*)
		echo "usage: backup.sh [daemon|now]" >&2
		exit 2
		;;
esac
