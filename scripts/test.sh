#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_DB_NAME="${TEST_DB_NAME:-dogn3_test}"
TEST_DATABASE_URL="${TEST_DATABASE_URL:-postgres:///${TEST_DB_NAME}}"
TEST_OUTPUT="$(mktemp)"

case "${TEST_DB_NAME}" in
    dogn | postgres | template0 | template1 | "")
        echo "Refusing to use unsafe test database name: ${TEST_DB_NAME}" >&2
        exit 1
        ;;
esac

cleanup_on_success() {
    dropdb --if-exists "${TEST_DB_NAME}"
}

cleanup_temp_file() {
    rm -f "${TEST_OUTPUT}"
}

summarize_test_output() {
    awk '
        /test result:/ {
            for (i = 1; i <= NF; i++) {
                if ($(i + 1) == "passed;") passed += $i;
                if ($(i + 1) == "failed;") failed += $i;
                if ($(i + 1) == "ignored;") skipped += $i;
                if ($(i + 1) == "filtered") filtered += $i;
            }
        }
        END {
            tested = passed + failed + skipped;
            printf "Test summary: %d tested, %d passed, %d failed, %d skipped/ignored, %d filtered out\n", tested, passed, failed, skipped, filtered;
        }
    ' "${TEST_OUTPUT}"
}

trap cleanup_temp_file EXIT

echo "Preparing test database: ${TEST_DB_NAME}"
dropdb --if-exists "${TEST_DB_NAME}"
createdb "${TEST_DB_NAME}"
echo "Created test database: ${TEST_DB_NAME}"

echo "Applying test schema"
psql "${TEST_DB_NAME}" -v ON_ERROR_STOP=1 -f "${ROOT_DIR}/tests/fixtures/schema.sql" >/dev/null

echo "Applying test fixture data"
psql "${TEST_DB_NAME}" -v ON_ERROR_STOP=1 -f "${ROOT_DIR}/tests/fixtures/home_data.sql" >/dev/null

echo "Running Rust tests"
if TEST_DATABASE_URL="${TEST_DATABASE_URL}" cargo test 2>&1 | tee "${TEST_OUTPUT}"; then
    summarize_test_output
    echo "Tests passed; dropping test database: ${TEST_DB_NAME}"
    cleanup_on_success
    echo "Test database dropped: ${TEST_DB_NAME}"
else
    status=$?
    summarize_test_output
    echo "Tests failed; keeping test database for diagnosis: ${TEST_DB_NAME}" >&2
    echo "Inspect with: psql ${TEST_DB_NAME}" >&2
    exit "${status}"
fi
