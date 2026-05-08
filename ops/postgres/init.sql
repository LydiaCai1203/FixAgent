CREATE TABLE IF NOT EXISTS projects (
    id BIGSERIAL PRIMARY KEY,
    project_key TEXT NOT NULL UNIQUE,
    project_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS pull_requests (
    id BIGSERIAL PRIMARY KEY,
    project_id BIGINT NOT NULL REFERENCES projects(id),
    platform TEXT NOT NULL,
    pr_number BIGINT NOT NULL,
    pr_url TEXT NOT NULL,
    latest_commit_sha TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(project_id, platform, pr_number)
);

CREATE TABLE IF NOT EXISTS review_runs (
    id BIGSERIAL PRIMARY KEY,
    pull_request_id BIGINT NOT NULL REFERENCES pull_requests(id),
    summary TEXT NOT NULL,
    recommendation TEXT NOT NULL,
    raw_report JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS issues (
    id BIGSERIAL PRIMARY KEY,
    pull_request_id BIGINT NOT NULL REFERENCES pull_requests(id),
    review_run_id BIGINT NOT NULL REFERENCES review_runs(id),
    fingerprint TEXT NOT NULL,
    severity TEXT NOT NULL,
    file_path TEXT NOT NULL,
    start_line BIGINT NOT NULL,
    end_line BIGINT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    suggestion TEXT NOT NULL,
    confidence INTEGER,
    status TEXT NOT NULL,
    source_bot TEXT NOT NULL,
    claimed_by TEXT,
    claimed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(pull_request_id, fingerprint)
);

CREATE TABLE IF NOT EXISTS fix_runs (
    id BIGSERIAL PRIMARY KEY,
    issue_id BIGINT NOT NULL REFERENCES issues(id),
    status TEXT NOT NULL,
    summary TEXT NOT NULL,
    rationale TEXT NOT NULL,
    verification_steps JSONB NOT NULL DEFAULT '[]'::jsonb,
    replacement_preview TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS verifications (
    id BIGSERIAL PRIMARY KEY,
    issue_id BIGINT NOT NULL REFERENCES issues(id),
    fix_run_id BIGINT NOT NULL REFERENCES fix_runs(id),
    status TEXT NOT NULL,
    summary TEXT NOT NULL,
    evidence JSONB NOT NULL DEFAULT '[]'::jsonb,
    gaps JSONB NOT NULL DEFAULT '[]'::jsonb,
    residual_risks JSONB NOT NULL DEFAULT '[]'::jsonb,
    next_actions JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS workflow_runs (
    id BIGSERIAL PRIMARY KEY,
    project_key TEXT NOT NULL,
    project_name TEXT NOT NULL,
    platform TEXT NOT NULL,
    pr_number BIGINT NOT NULL,
    pr_url TEXT NOT NULL,
    status TEXT NOT NULL,
    stop_reason TEXT,
    max_rounds INTEGER NOT NULL,
    summary TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS workflow_rounds (
    id BIGSERIAL PRIMARY KEY,
    workflow_run_id BIGINT NOT NULL REFERENCES workflow_runs(id),
    round_number INTEGER NOT NULL,
    review_run_id BIGINT,
    issue_id BIGINT REFERENCES issues(id),
    fix_run_id BIGINT REFERENCES fix_runs(id),
    verification_id BIGINT REFERENCES verifications(id),
    status TEXT NOT NULL,
    stop_reason TEXT,
    summary TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    UNIQUE(workflow_run_id, round_number)
);

CREATE INDEX IF NOT EXISTS idx_pull_requests_project_id ON pull_requests(project_id);
CREATE INDEX IF NOT EXISTS idx_review_runs_pull_request_id ON review_runs(pull_request_id);
CREATE INDEX IF NOT EXISTS idx_issues_pull_request_id ON issues(pull_request_id);
CREATE INDEX IF NOT EXISTS idx_issues_review_run_id ON issues(review_run_id);
CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(status);
CREATE INDEX IF NOT EXISTS idx_fix_runs_issue_id ON fix_runs(issue_id);
CREATE INDEX IF NOT EXISTS idx_verifications_issue_id ON verifications(issue_id);
CREATE INDEX IF NOT EXISTS idx_verifications_fix_run_id ON verifications(fix_run_id);
CREATE INDEX IF NOT EXISTS idx_workflow_runs_project_key ON workflow_runs(project_key);
CREATE INDEX IF NOT EXISTS idx_workflow_rounds_workflow_run_id ON workflow_rounds(workflow_run_id);
