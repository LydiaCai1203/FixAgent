#!/bin/bash
# Database migration runner
# Executes all SQL migration files in order

set -e

MIGRATIONS_DIR="${MIGRATIONS_DIR:-./ops/postgres/migrations}"
DB_URL="${DATABASE_URL:-postgres://fixagent:fixagent@localhost:5432/fixagent}"

echo "Running migrations from $MIGRATIONS_DIR"
echo "Database: $DB_URL"

# Create migrations tracking table if not exists
psql "$DB_URL" -c "
CREATE TABLE IF NOT EXISTS schema_migrations (
    id SERIAL PRIMARY KEY,
    filename TEXT NOT NULL UNIQUE,
    executed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
" 2>/dev/null || {
    echo "Error: Could not connect to database"
    exit 1
}

# Run each migration file in order
for file in $(ls -1 "$MIGRATIONS_DIR"/*.sql 2>/dev/null | sort); do
    filename=$(basename "$file")
    
    # Check if already executed
    already_executed=$(psql "$DB_URL" -t -c "SELECT 1 FROM schema_migrations WHERE filename = '$filename'" 2>/dev/null | xargs)
    
    if [ "$already_executed" = "1" ]; then
        echo "  SKIP $filename (already executed)"
        continue
    fi
    
    echo "  RUN  $filename"
    psql "$DB_URL" -f "$file"
    
    # Record migration
    psql "$DB_URL" -c "INSERT INTO schema_migrations (filename) VALUES ('$filename')"
done

echo "Migrations complete"
