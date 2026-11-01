#!/bin/sh
# One-shot server setup for a fresh Ubuntu 24.04 VPS (run as root, from deploy/):
#
#   cd /opt/sowling/deploy && sh bootstrap.sh
#
# Installs Docker and the firewall, creates .env, generates every secret
# directly on the server (nothing secret is printed), builds and starts the
# stack. Safe to run again: values already in .env are never overwritten.
#
# Left for a human afterwards (see docs/SERVER_SETUP.md):
#   - copy the backup private key into a password manager, then delete it
#   - Stripe keys and price IDs, DESKTOP_DOWNLOAD_URL
set -eu

cd "$(dirname "$0")"
[ "$(id -u)" = 0 ] || { echo "run as root" >&2; exit 1; }

say() { printf '\n=== %s\n' "$*"; }

# Sets KEY=value in .env only if KEY is still empty.
set_if_empty() {
	key="$1"; value="$2"
	if grep -q "^${key}=$" .env; then
		sed -i "s|^${key}=$|${key}=${value}|" .env
		echo "  ${key}: set"
	else
		echo "  ${key}: already set, kept"
	fi
}

say "System packages, Docker, firewall"
export DEBIAN_FRONTEND=noninteractive
apt-get update -q
apt-get upgrade -y -q
apt-get install -y -q age git ufw openssl curl
command -v docker >/dev/null 2>&1 || curl -fsSL https://get.docker.com | sh
ufw allow OpenSSH >/dev/null
ufw allow 80/tcp >/dev/null
ufw allow 443/tcp >/dev/null
ufw allow 443/udp >/dev/null
ufw --force enable >/dev/null
echo "  firewall: SSH, 80, 443 open"

# Compiling the Rust backend needs a few GB of memory.
if [ ! -f /swapfile ] && [ "$(awk '/MemTotal/ {print int($2/1024)}' /proc/meminfo)" -lt 6000 ]; then
	say "Adding 4 GB swap for the build"
	fallocate -l 4G /swapfile && chmod 600 /swapfile && mkswap /swapfile >/dev/null && swapon /swapfile
	grep -q '^/swapfile' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab
fi

say ".env"
if [ ! -f .env ]; then
	cp .env.example .env
	echo "  created from .env.example"
fi
chmod 600 .env
set_if_empty POSTGRES_PASSWORD "$(openssl rand -hex 32)"
set_if_empty JWT_SECRET "$(openssl rand -hex 48)"

# Backup key pair: the public key goes into .env, the private key into a
# root-only file that a human must move into a password manager.
if grep -q '^BACKUP_AGE_RECIPIENT=$' .env; then
	umask 077
	age-keygen -o /root/sowling-backup-key.txt 2>/dev/null
	set_if_empty BACKUP_AGE_RECIPIENT "$(age-keygen -y /root/sowling-backup-key.txt)"
	BACKUP_KEY_NEW=1
fi

say "Building the backend (takes 5-15 minutes the first time)"
docker compose build backend

keys=$(docker compose run --rm --no-deps -T backend gen-license-keys)
set_if_empty LICENSE_SIGNING_KEY "$(echo "$keys" | sed -n 's/^LICENSE_SIGNING_KEY=//p')"
# Only valid if this run generated the signing key; re-derive otherwise.
public_key=$(echo "$keys" | sed -n 's/^SOWLING_LICENSE_PUBLIC_KEY=//p')
if [ "$(grep '^LICENSE_SIGNING_KEY=' .env | cut -d= -f2-)" = "$(echo "$keys" | sed -n 's/^LICENSE_SIGNING_KEY=//p')" ]; then
	echo "$public_key" > /root/sowling-license-public-key.txt
fi

vapid=$(docker compose run --rm --no-deps -T backend gen-vapid-keys)
set_if_empty VAPID_PRIVATE_KEY "$(echo "$vapid" | sed -n 's/^VAPID_PRIVATE_KEY=//p')"

say "Starting all services"
docker compose up -d --build
echo "  waiting for the backend to become healthy ..."
i=0
until [ "$(docker inspect -f '{{.State.Health.Status}}' "$(docker compose ps -q backend)" 2>/dev/null)" = "healthy" ]; do
	i=$((i + 1)); [ "$i" -gt 60 ] && { echo "  backend not healthy after 5 minutes:"; docker compose logs --tail 40 backend; exit 1; }
	sleep 5
done
docker compose ps

api=$(grep '^API_DOMAIN=' .env | cut -d= -f2-)
app=$(grep '^APP_DOMAIN=' .env | cut -d= -f2-)
say "Done"
echo "  Web app:  https://$app"
echo "  API:      https://$api/health   (first HTTPS request may take a minute: certificate)"
echo
echo "  Still to do by a human:"
if [ "${BACKUP_KEY_NEW:-0}" = 1 ]; then
	echo "  1. Backup private key: cat /root/sowling-backup-key.txt"
	echo "     copy it into a password manager, then: shred -u /root/sowling-backup-key.txt"
fi
[ -f /root/sowling-license-public-key.txt ] && echo "  2. Desktop build key (public, not secret): cat /root/sowling-license-public-key.txt"
echo "  3. Back up LICENSE_SIGNING_KEY and VAPID_PRIVATE_KEY from deploy/.env in a password manager"
echo "  4. Stripe values and DESKTOP_DOWNLOAD_URL in deploy/.env, then: docker compose up -d backend"
