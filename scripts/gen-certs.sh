#!/bin/sh
# Private root CA restricted to one parent domain, and a server certificate signed by it.
#
#   certs/forever.cert.pem         leaf + CA chain  -> bnetserver (see below)
#   certs/forever.key.pem          server private key -> bnetserver, NEVER share
#   certs/ca/forever_root_ca.crt   CA certificate -> players and the launcher, public
#   certs/ca/forever_root_ca.key   CA private key -> NEVER share, NEVER commit
#
# HOSTNAME is the login (LoginREST) hostname your client connects to (CN and SAN).
# The CA nameConstraints permit only CA_DOMAIN and its subdomains. CA_DOMAIN defaults to
# the parent domain of HOSTNAME (everything after the first dot); set it in the
# environment to override. An existing CA is reused and cannot change its constraints.
#
# Usage:
#   ./scripts/gen-certs.sh <hostname> [days]
#   ./scripts/gen-certs.sh <days> <hostname>
set -eu

is_number() {
    case "$1" in
        ''|*[!0-9]*) return 1 ;;
        *) return 0 ;;
    esac
}

if [ "${1:-}" != "" ] && is_number "$1"; then
    DAYS="$1"
    HOST="${2:-}"
else
    HOST="${1:-}"
    DAYS="${2:-825}"
fi

if [ -z "$HOST" ]; then
    echo "usage: $0 <hostname> [days]   (the login hostname your client connects to)" >&2
    exit 1
fi

if ! is_number "$DAYS" || [ "$DAYS" -eq 0 ]; then
    echo "days must be a positive integer" >&2
    exit 1
fi
case "$HOST" in
    ''|*[!A-Za-z0-9.-]*|.*|*.|*-|*.)
        echo "hostname must be a DNS name (letters, digits, dots, hyphens)" >&2
        exit 1
        ;;
esac
case "$HOST" in
    *..*) echo "hostname must not contain '..'" >&2; exit 1 ;;
esac
CA_DOMAIN="${CA_DOMAIN:-${HOST#*.}}"
case "$HOST" in
    "$CA_DOMAIN"|*."$CA_DOMAIN") ;;
    *)
        echo "hostname must be $CA_DOMAIN or a name under it (CA nameConstraints)." >&2
        echo "Set CA_DOMAIN before the CA is created if you need another parent domain." >&2
        exit 1
        ;;
esac

DIR="$(cd "$(dirname "$0")/.." && pwd)/certs"
CA="$DIR/ca"
mkdir -p "$CA"
umask 077

if [ ! -f "$CA/forever_root_ca.key" ]; then
    echo "== new root CA (permitted DNS: $CA_DOMAIN)"
    openssl req -x509 -newkey rsa:4096 -sha256 -nodes -days 3650 \
        -keyout "$CA/forever_root_ca.key" -out "$CA/forever_root_ca.crt" \
        -subj "/CN=Forever Private Root CA" \
        -addext "basicConstraints=critical,CA:TRUE,pathlen:0" \
        -addext "keyUsage=critical,keyCertSign,cRLSign" \
        -addext "nameConstraints=critical,permitted;DNS:$CA_DOMAIN"
else
    echo "== reusing existing root CA $CA/forever_root_ca.crt"
fi

echo "== server certificate for $HOST ($DAYS days)"
openssl req -newkey rsa:2048 -sha256 -nodes \
    -keyout "$DIR/forever.key.pem" -out "$CA/server.csr" -subj "/CN=$HOST"

cat > "$CA/server.ext" <<EXT
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
subjectAltName=DNS:$HOST,DNS:*.$CA_DOMAIN
EXT

openssl x509 -req -in "$CA/server.csr" -CA "$CA/forever_root_ca.crt" -CAkey "$CA/forever_root_ca.key" \
    -CAcreateserial -days "$DAYS" -sha256 -extfile "$CA/server.ext" -out "$CA/server.crt"

cat "$CA/server.crt" "$CA/forever_root_ca.crt" > "$DIR/forever.cert.pem"
rm -f "$CA/server.csr" "$CA/server.ext"
chmod 644 "$DIR/forever.cert.pem" "$CA/forever_root_ca.crt"
chmod 600 "$DIR/forever.key.pem" "$CA/forever_root_ca.key"

openssl verify -CAfile "$CA/forever_root_ca.crt" "$CA/server.crt"
echo
echo "Done."
echo "Copy onto the bnetserver runtime (not a build folder that CMake POST_BUILD may overwrite):"
echo "  $DIR/forever.cert.pem  ->  bnetserver.cert.pem"
echo "  $DIR/forever.key.pem   ->  bnetserver.key.pem"
echo "Give players ONLY:"
echo "  $CA/forever_root_ca.crt"
echo "NEVER share or commit:"
echo "  $CA/forever_root_ca.key"
echo "Point the launcher at this CA: replace app/src-tauri/resources/forever_root_ca.crt"
echo "with that .crt, or set FOREVER_CA_CERT to its PEM text, then rebuild."
echo "See docs/OPERATOR.md."
