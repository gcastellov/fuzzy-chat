#!/bin/bash

set -e

DOMAIN="${1:-your.domain.controller.com}"
CERTS_DIR="${2:-$(dirname "$0")}"

mkdir -p "$CERTS_DIR"
cd "$CERTS_DIR"

CA_KEY="ca.key"
CA_CERT="ca.crt"
SERVER_KEY="server.key"
SERVER_CSR="dc.csr"
SERVER_CERT="server.crt"
EXT_FILE="dc.ext"
DAYS_CA=3650
DAYS_SERVER=825
CA_SUBJ="/C=US/ST=State/L=City/O=ExampleOrg/OU=IT Department/CN=$DOMAIN"
SERVER_SUBJ="/C=US/ST=State/L=City/O=ExampleOrg/OU=IT Department/CN=$DOMAIN"
SERVER_DNS="$DOMAIN"

echo "Generating CA private key..."
openssl genrsa -out "$CA_KEY" 4096

echo "Generating CA certificate..."
openssl req -x509 -new -nodes -key "$CA_KEY" -sha256 -days $DAYS_CA -out "$CA_CERT" -subj "$CA_SUBJ"

echo "Generating server private key..."
openssl genrsa -out "$SERVER_KEY" 2048

echo "Generating server CSR..."
openssl req -new -key "$SERVER_KEY" -out "$SERVER_CSR" -subj "$SERVER_SUBJ"

echo "Creating SAN config file..."
cat > "$EXT_FILE" <<EOF
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
DNS.1 = $SERVER_DNS
EOF

echo "Signing server certificate with CA..."
openssl x509 -req -in "$SERVER_CSR" -CA "$CA_CERT" -CAkey "$CA_KEY" -CAcreateserial -out "$SERVER_CERT" -days $DAYS_SERVER -sha256 -extfile "$EXT_FILE"

echo "Certificates generated in $CERTS_DIR:"
ls -l "$CA_KEY" "$CA_CERT" "$SERVER_KEY" "$SERVER_CERT"

echo