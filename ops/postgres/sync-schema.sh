#!/bin/bash
# Database schema sync script
# Ensures database schema matches init.sql by running ALTER statements

set -e

DB_URL="${DATABASE_URL:-postgres://fixagent:fixagent@localhost:5432/fixagent}"

echo "Syncing database schema..."

# Ensure issues table has all required columns
psql "$DB_URL" -c "
DO \$\$
BEGIN
    -- Add suggestion_code if missing
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'issues' AND column_name = 'suggestion_code'
    ) THEN
        ALTER TABLE issues ADD COLUMN suggestion_code TEXT;
        RAISE NOTICE 'Added suggestion_code column to issues table';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'issues' AND column_name = 'original_code'
    ) THEN
        ALTER TABLE issues ADD COLUMN original_code TEXT;
        RAISE NOTICE 'Added original_code column to issues table';
    END IF;

    -- Add repo_url to projects if missing
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'projects' AND column_name = 'repo_url'
    ) THEN
        ALTER TABLE projects ADD COLUMN repo_url TEXT;
        RAISE NOTICE 'Added repo_url column to projects table';
    END IF;

    -- Add repo_dir to projects if missing
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'projects' AND column_name = 'repo_dir'
    ) THEN
        ALTER TABLE projects ADD COLUMN repo_dir TEXT;
        RAISE NOTICE 'Added repo_dir column to projects table';
    END IF;
END
\$\$;
"

echo "Schema sync complete"
