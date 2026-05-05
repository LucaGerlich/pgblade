#!/usr/bin/env bash
# Manage the pgblade-test Postgres container for integration tests.
#
# Usage:
#   ./scripts/test-db.sh start   — Start the test container
#   ./scripts/test-db.sh stop    — Stop and remove the container
#   ./scripts/test-db.sh status  — Check if running

set -euo pipefail

CONTAINER_NAME="pgblade-test"
POSTGRES_PORT="15432"
POSTGRES_USER="pgblade_test"
POSTGRES_PASSWORD="pgblade_test"
POSTGRES_DB="pgblade_test"

start() {
    if docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        echo "Container ${CONTAINER_NAME} is already running on port ${POSTGRES_PORT}"
        return 0
    fi

    # Remove stopped container if it exists
    docker rm -f "${CONTAINER_NAME}" 2>/dev/null || true

    echo "Starting ${CONTAINER_NAME} on port ${POSTGRES_PORT}..."
    docker run -d \
        --name "${CONTAINER_NAME}" \
        -p "${POSTGRES_PORT}:5432" \
        -e POSTGRES_USER="${POSTGRES_USER}" \
        -e POSTGRES_PASSWORD="${POSTGRES_PASSWORD}" \
        -e POSTGRES_DB="${POSTGRES_DB}" \
        postgres:16-alpine

    # Wait for Postgres to be ready
    echo -n "Waiting for Postgres to accept connections"
    for i in $(seq 1 30); do
        if docker exec "${CONTAINER_NAME}" pg_isready -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" >/dev/null 2>&1; then
            echo " ready!"
            return 0
        fi
        echo -n "."
        sleep 1
    done

    echo " TIMEOUT"
    exit 1
}

stop() {
    echo "Stopping ${CONTAINER_NAME}..."
    docker rm -f "${CONTAINER_NAME}" 2>/dev/null || true
    echo "Done."
}

status() {
    if docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        echo "Running on port ${POSTGRES_PORT}"
    else
        echo "Not running"
        exit 1
    fi
}

case "${1:-}" in
    start)  start ;;
    stop)   stop ;;
    status) status ;;
    *)
        echo "Usage: $0 {start|stop|status}"
        exit 1
        ;;
esac
