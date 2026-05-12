export type ProjectSummary = {
  id: number;
  project_key: string;
  project_name: string;
  repo_url: string | null;
  repo_dir: string | null;
  created_at: string;
  updated_at: string;
};

export type PullRequestSummary = {
  id: number;
  project_id: number;
  platform: string;
  pr_number: number;
  pr_url: string;
  latest_commit_sha: string | null;
  status: string;
  created_at: string;
  updated_at: string;
};

export type IssueSummary = {
  id: number;
  project_key: string;
  project_name: string;
  platform: string;
  pr_number: number;
  pull_request_id: number;
  review_run_id: number;
  severity: string;
  file_path: string;
  start_line: number;
  end_line: number;
  title: string;
  description: string;
  suggestion: string;
  suggestion_code: string | null;
  original_code: string | null;
  status: string;
  confidence: number | null;
  created_at: string;
  updated_at: string;
  fix_replacement_preview: string | null;
  fix_commit_sha: string | null;
  pr_url: string;
};

export type WorkflowRunSummary = {
  id: number;
  project_key: string;
  project_name: string;
  platform: string;
  pr_number: number;
  pr_url: string;
  status: string;
  stop_reason: string | null;
  max_rounds: number;
  summary: string | null;
  started_at: string;
  completed_at: string | null;
};

export type WorkflowRoundSummary = {
  id: number;
  workflow_run_id: number;
  round_number: number;
  review_run_id: number | null;
  issue_id: number | null;
  fix_run_id: number | null;
  verification_id: number | null;
  status: string;
  stop_reason: string | null;
  summary: string | null;
  started_at: string;
  completed_at: string | null;
};
