#!/bin/bash

set -e

DOMAINS=("controller" "proxy-one" "proxy-two" "proxy-three" "proxy-four" "proxy-five" "client-one" "client-two" "client-three" "client-four")

SCRIPT_DIR=$(dirname "$0")
GEN_SCRIPT="$SCRIPT_DIR/generate_certs.sh"

for DOMAIN in "${DOMAINS[@]}"; do
    CERT_DIR="$SCRIPT_DIR/$DOMAIN"
    mkdir -p "$CERT_DIR"
    echo "Generating certificates for $DOMAIN in $CERT_DIR"
    bash "$GEN_SCRIPT" "$DOMAIN" "$CERT_DIR"
done

echo "All certificates generated."