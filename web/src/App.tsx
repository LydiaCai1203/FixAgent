import { useEffect, useMemo, useState } from 'react';

const API_BASE_URL = (import.meta.env.VITE_API_BASE_URL || `${window.location.origin}/api`).replace(/\/$/, '');

type ProjectSummary = {
  id: number;
  project_key: string;
  project_name: string;
  created_at: string;
  updated_at: string;
};

type PullRequestSummary = {
  id: number;
  project_id: number;
  platform: string;
  pr_number: number;
  pr_url: string;
  latest_commit_sha: string | null;
  created_at: string;
  updated_at: string;
};

type IssueSummary = {
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
  status: string;
  confidence: number | null;
  created_at: string;
  updated_at: string;
};

type WorkflowRunSummary = {
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

type WorkflowRoundSummary = {
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

export default function App() {
  const [isProjectMenuOpen, setIsProjectMenuOpen] = useState(true);
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [selectedProjectKey, setSelectedProjectKey] = useState<string | null>(null);
  const [prs, setPrs] = useState<PullRequestSummary[]>([]);
  const [selectedPrId, setSelectedPrId] = useState<number | null>(null);
  const [projectIssues, setProjectIssues] = useState<IssueSummary[]>([]);
  const [isLoadingProjects, setIsLoadingProjects] = useState(true);
  const [isLoadingPrs, setIsLoadingPrs] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showCreateProject, setShowCreateProject] = useState(false);
  const [newProjectName, setNewProjectName] = useState('');
  const [showCreatePr, setShowCreatePr] = useState(false);
  const [newPrUrl, setNewPrUrl] = useState('');
  const [pendingProjectKeyForPr, setPendingProjectKeyForPr] = useState<string | null>(null);
  const [openProjectMenuKey, setOpenProjectMenuKey] = useState<string | null>(null);
  const [hoveredReviewPrId, setHoveredReviewPrId] = useState<number | null>(null);
  const [workflowRuns, setWorkflowRuns] = useState<WorkflowRunSummary[]>([]);
  const [reviewRunningPrIds, setReviewRunningPrIds] = useState<number[]>([]);
  const [workflowRounds, setWorkflowRounds] = useState<WorkflowRoundSummary[]>([]);
  const [pendingReviewPrIds, setPendingReviewPrIds] = useState<number[]>([]);

  const selectedProject = useMemo(
    () => projects.find((project) => project.project_key === selectedProjectKey) ?? null,
    [projects, selectedProjectKey],
  );

  const selectedPr = useMemo(
    () => prs.find((pr) => pr.id === selectedPrId) ?? null,
    [prs, selectedPrId],
  );

  const prIdentityById = useMemo(() => {
    const map = new Map<number, string>();
    for (const pr of prs) {
      map.set(pr.id, `${pr.platform}:${pr.pr_number}`);
    }
    return map;
  }, [prs]);

  const inferredProjectName = useMemo(() => {
    if (selectedPr) {
      return deriveProjectNameFromPrUrl(selectedPr.pr_url) ?? selectedProject?.project_name ?? 'Project';
    }
    return selectedProject?.project_name ?? 'Select a project';
  }, [selectedPr, selectedProject?.project_name]);

  const projectMetrics = useMemo(() => {
    if (!selectedProjectKey) {
      return { prs: 0, bugs: 0, open: 0 };
    }

    const bugs = projectIssues.length;
    const open = projectIssues.filter((issue) => ['open', 'reopened', 'needs_human'].includes(issue.status)).length;
    return { prs: prs.length, bugs, open };
  }, [selectedProjectKey, prs.length, projectIssues]);

  const prIssueSummaryMap = useMemo(() => {
    const summary = new Map<number, {
      total: number;
      open: number;
      needsHuman: number;
      resolved: number;
    }>();

    for (const issue of projectIssues) {
      const current = summary.get(issue.pr_number) ?? { total: 0, open: 0, needsHuman: 0, resolved: 0 };
      current.total += 1;
      if (issue.status === 'open' || issue.status === 'reopened') {
        current.open += 1;
      }
      if (issue.status === 'needs_human') {
        current.needsHuman += 1;
      }
      if (issue.status === 'resolved') {
        current.resolved += 1;
      }
      summary.set(issue.pr_number, current);
    }

    return summary;
  }, [projectIssues]);

  useEffect(() => {
    void loadProjects();
  }, []);

  useEffect(() => {
    if (!selectedProjectKey) {
      setPrs([]);
      setProjectIssues([]);
      setSelectedPrId(null);
      setWorkflowRuns([]);
      setWorkflowRounds([]);
      return;
    }

    void loadPrs(selectedProjectKey);
    void loadProjectIssues(selectedProjectKey, selectedPr);
    void loadWorkflows(selectedProjectKey);
  }, [selectedProjectKey]);

  useEffect(() => {
    if (!selectedProjectKey) {
      return;
    }

    void loadProjectIssues(selectedProjectKey, selectedPr);
    void loadSelectedWorkflowRounds(selectedPr);
  }, [selectedProjectKey, selectedPrId]);

  useEffect(() => {
    if (!selectedProjectKey) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void loadProjectIssues(selectedProjectKey, selectedPr);
      void loadWorkflows(selectedProjectKey);
      void loadSelectedWorkflowRounds(selectedPr);
    }, 1000);

    return () => window.clearInterval(intervalId);
  }, [selectedProjectKey, selectedPrId]);

  async function loadProjects() {
    setIsLoadingProjects(true);
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/projects`);
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }
      const data = (await response.json()) as ProjectSummary[];
      setProjects(data);

      if (data.length === 0) {
        setSelectedProjectKey(null);
        return;
      }

      const nextProject = data.find((project) => project.project_key === selectedProjectKey) ?? data[0];
      setSelectedProjectKey(nextProject.project_key);
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setIsLoadingProjects(false);
    }
  }

  async function loadPrs(projectKey: string) {
    setIsLoadingPrs(true);
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/prs?project_key=${encodeURIComponent(projectKey)}`);
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }
      const data = (await response.json()) as PullRequestSummary[];
      setPrs(data);

      if (data.length === 0) {
        setSelectedPrId(null);
        return;
      }

      const nextPr = data.find((pr) => pr.id === selectedPrId) ?? data[0];
      setSelectedPrId(nextPr.id);
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setIsLoadingPrs(false);
    }
  }

  async function loadProjectIssues(projectKey: string, pr?: PullRequestSummary | null) {
    try {
      const params = new URLSearchParams({ project_key: projectKey });
      if (pr) {
        params.set('platform', pr.platform);
        params.set('pr_number', String(pr.pr_number));
      }

      const response = await fetch(`${API_BASE_URL}/issues?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }
      const data = (await response.json()) as IssueSummary[];
      setProjectIssues(data);
    } catch (err) {
      setError(toErrorMessage(err));
      setProjectIssues([]);
    }
  }

  async function loadWorkflows(projectKey: string) {
    try {
      const response = await fetch(`${API_BASE_URL}/workflows?project_key=${encodeURIComponent(projectKey)}`);
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }
      const data = (await response.json()) as WorkflowRunSummary[];
      setWorkflowRuns(data);
      const runningPrNumbers = new Set(
        data.filter((item) => item.status === 'running').map((item) => `${item.platform}:${item.pr_number}`),
      );
      setReviewRunningPrIds(
        Array.from(prIdentityById.entries())
          .filter(([, identity]) => runningPrNumbers.has(identity))
          .map(([prId]) => prId),
      );
      setPendingReviewPrIds((current) => current.filter((prId) => !prIdentityById.has(prId) || !runningPrNumbers.has(prIdentityById.get(prId) ?? '')));
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }

  async function loadSelectedWorkflowRounds(pr?: PullRequestSummary | null) {
    if (!pr) {
      setWorkflowRounds([]);
      return;
    }

    const workflow = workflowStatusForPr(pr);
    if (!workflow) {
      setWorkflowRounds([]);
      return;
    }

    try {
      const response = await fetch(`${API_BASE_URL}/workflows/${workflow.id}/rounds`);
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }
      const data = (await response.json()) as WorkflowRoundSummary[];
      setWorkflowRounds(data);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }

  function openSelectedPr() {
    if (!selectedPr) {
      return;
    }
    window.open(selectedPr.pr_url, '_blank', 'noopener,noreferrer');
  }

  async function handleCreateProject() {
    if (!newProjectName.trim()) return;
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/projects`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ project_name: newProjectName.trim() }),
      });
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }
      await loadProjects();
      setShowCreateProject(false);
      setNewProjectName('');
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }

  async function handleCreatePr() {
    if (!pendingProjectKeyForPr || !newPrUrl.trim()) return;
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/prs`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ project_key: pendingProjectKeyForPr, pr_url: newPrUrl.trim() }),
      });
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }

      setSelectedProjectKey(pendingProjectKeyForPr);
      await loadPrs(pendingProjectKeyForPr);
      await loadProjectIssues(pendingProjectKeyForPr, selectedPr);
      setShowCreatePr(false);
      setNewPrUrl('');
      setPendingProjectKeyForPr(null);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }

  async function handleDeleteProject(project: ProjectSummary) {
    const confirmed = window.confirm(`Delete project "${project.project_name}"?`);
    if (!confirmed) {
      return;
    }

    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/projects`, {
        method: 'DELETE',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ project_key: project.project_key }),
      });
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }

      if (selectedProjectKey === project.project_key) {
        setSelectedProjectKey(null);
        setSelectedPrId(null);
      }
      setOpenProjectMenuKey(null);
      await loadProjects();
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }

  async function handleRunReview(pr: PullRequestSummary) {
    const project = projects.find((item) => item.id === pr.project_id);
    if (!project) {
      setError('Project not found for this PR');
      return;
    }

    setError(null);
    setPendingReviewPrIds((current) => (current.includes(pr.id) ? current : [...current, pr.id]));
    try {
      const response = await fetch(`${API_BASE_URL}/reviews`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          project_key: project.project_key,
          project_name: project.project_name,
          pr_url: pr.pr_url,
        }),
      });
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }

      setSelectedProjectKey(project.project_key);
      setSelectedPrId(pr.id);
      await loadWorkflows(project.project_key);
      await loadSelectedWorkflowRounds(pr);
    } catch (err) {
      setPendingReviewPrIds((current) => current.filter((item) => item !== pr.id));
      setError(toErrorMessage(err));
    }
  }

  function workflowStatusForPr(pr: PullRequestSummary) {
    return workflowRuns.find((item) => item.platform === pr.platform && item.pr_number === pr.pr_number) ?? null;
  }

  function reviewDetailsForPr(pr: PullRequestSummary) {
    const workflow = workflowStatusForPr(pr);
    const relatedRounds = workflow ? workflowRounds.filter((round) => round.workflow_run_id === workflow.id) : [];

    return {
      workflow,
      relatedRounds,
      isPendingSubmission: pendingReviewPrIds.includes(pr.id),
    };
  }

  return (
    <div className="brew-shell">
      <aside className="brew-sidebar">
        <div className="brew-brand">
          <div className="brew-brand-mark">
            <img className="brew-brand-image" src="/monkeycode1.png" alt="MonkeyCode" />
          </div>
          <div>
            <div className="brew-brand-kicker">FixAgent Console</div>
            <h1>PR Review Board</h1>
          </div>
        </div>

        <nav className="brew-nav">
          <button className="brew-nav-item-add" onClick={() => { setNewProjectName(''); setShowCreateProject(true); }}>
            <span className="brew-nav-label">+ Add Project</span>
          </button>
          <button className={isProjectMenuOpen ? 'brew-nav-item brew-nav-item-active' : 'brew-nav-item'} onClick={() => setIsProjectMenuOpen((value) => !value)}>
            <span className="brew-nav-label">Project</span>
            <span className="brew-nav-caret">{isProjectMenuOpen ? '-' : '+'}</span>
          </button>
        </nav>

        {isProjectMenuOpen ? <section className="brew-sidebar-section brew-sidebar-subsection">
          <div className="brew-project-list">
            {projects.map((project) => (
              <div
                key={project.project_key}
                className={project.project_key === selectedProjectKey ? 'brew-project-chip brew-project-chip-active' : 'brew-project-chip'}
              >
                <button
                  className="brew-project-chip-main"
                  onClick={() => {
                    setSelectedProjectKey(project.project_key);
                  }}
                >
                  <span className="brew-project-branch" />
                  <div className="brew-project-chip-title">{project.project_name}</div>
                  <div className="brew-project-chip-meta">{project.project_key}</div>
                </button>
                <div className="brew-project-chip-menu-wrap">
                  <button
                    className="brew-project-chip-menu-button"
                    onClick={() => {
                      setOpenProjectMenuKey((current) => current === project.project_key ? null : project.project_key);
                    }}
                    aria-label={`Project actions for ${project.project_name}`}
                  >
                    ...
                  </button>
                  {openProjectMenuKey === project.project_key ? (
                    <div className="brew-project-chip-menu">
                      <button
                        className="brew-project-chip-menu-item"
                        onClick={() => {
                          setPendingProjectKeyForPr(project.project_key);
                          setNewPrUrl('');
                          setShowCreatePr(true);
                          setOpenProjectMenuKey(null);
                        }}
                      >
                        Add PR
                      </button>
                      <button
                        className="brew-project-chip-menu-item brew-project-chip-menu-item-danger"
                        onClick={() => {
                          void handleDeleteProject(project);
                        }}
                      >
                        Delete Project
                      </button>
                    </div>
                  ) : null}
                </div>
              </div>
            ))}
            {projects.length === 0 && !isLoadingProjects ? <div className="brew-empty-mini">No projects yet.</div> : null}
          </div>
        </section> : null}
      </aside>

      <main className="brew-main">
        <header className="brew-topbar">
        </header>

        <section className="brew-hero-card">
          <div className="brew-hero-stats">
            <div className="brew-stat-inline">
              <span>Projects</span>
              <strong>{String(projects.length).padStart(2, '0')}</strong>
            </div>
            <div className="brew-stat-inline">
              <span>PRs</span>
              <strong>{String(projectMetrics.prs).padStart(2, '0')}</strong>
            </div>
            <div className="brew-stat-inline">
              <span>Open Bugs</span>
              <strong>{String(projectMetrics.open).padStart(2, '0')}</strong>
            </div>
          </div>
        </section>

        <section className="brew-prpool-layout">
          <section className="brew-panel brew-prpool-panel">
            <div className="brew-panel-header">
              <div>
                <h3>PR Pool</h3>
              </div>
            </div>

            <div className="brew-card-grid brew-pr-card-grid">
              {prs.map((pr) => {
                const summary = prIssueSummaryMap.get(pr.pr_number) ?? { total: 0, open: 0, needsHuman: 0, resolved: 0 };
                const { workflow, relatedRounds, isPendingSubmission } = reviewDetailsForPr(pr);
                const isReviewBusy = pendingReviewPrIds.includes(pr.id) || reviewRunningPrIds.includes(pr.id);
                return (
                  <div
                    key={pr.id}
                    className={pr.id === selectedPrId ? 'brew-card brew-card-active brew-pr-card' : 'brew-card brew-pr-card'}
                    onClick={() => setSelectedPrId(pr.id)}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault();
                        setSelectedPrId(pr.id);
                      }
                    }}
                  >
                      <div className="brew-card-header brew-pr-card-header">
                        <div>
                          <div className="brew-card-kicker">{pr.platform}</div>
                          <strong>PR #{pr.pr_number}</strong>
                        </div>
                        <div
                          className="brew-pr-card-actions"
                          onMouseEnter={() => setHoveredReviewPrId(pr.id)}
                          onMouseLeave={() => setHoveredReviewPrId((current) => current === pr.id ? null : current)}
                        >
                          <button
                            className="brew-pr-run-button"
                            onClick={(event) => {
                              event.stopPropagation();
                              void handleRunReview(pr);
                            }}
                            disabled={isReviewBusy}
                          >
                            {isReviewBusy ? <span className="brew-pr-run-spinner" aria-hidden="true" /> : 'R'}
                          </button>
                          {hoveredReviewPrId === pr.id ? (
                            <div className="brew-review-popover">
                              {isPendingSubmission ? <div className="brew-review-popover-line">Submitting review task...</div> : null}
                              {!workflow && !isPendingSubmission ? <div className="brew-review-popover-line">No review task has started yet.</div> : null}
                              {workflow ? (
                                <>
                                  <div className="brew-review-popover-line"><strong>Status:</strong> {workflow.status}</div>
                                  <div className="brew-review-popover-line"><strong>Started:</strong> {formatDateTime(workflow.started_at)}</div>
                                  <div className="brew-review-popover-line"><strong>Completed:</strong> {workflow.completed_at ? formatDateTime(workflow.completed_at) : 'Running'}</div>
                                  <div className="brew-review-popover-line"><strong>Summary:</strong> {workflow.summary ?? 'No workflow summary yet.'}</div>
                                  {relatedRounds.length > 0 ? (
                                    <div className="brew-review-popover-rounds">
                                      {relatedRounds.map((round) => (
                                        <div key={round.id} className="brew-review-popover-round">
                                          <div className="brew-review-popover-line"><strong>Round {round.round_number}</strong> {round.status}</div>
                                          <div className="brew-review-popover-line">{round.summary ?? 'No round summary yet.'}</div>
                                          <div className="brew-review-popover-line">{round.completed_at ? `Completed ${formatDateTime(round.completed_at)}` : 'Running'}</div>
                                        </div>
                                      ))}
                                    </div>
                                  ) : null}
                                </>
                              ) : null}
                            </div>
                          ) : null}
                        </div>
                      </div>
                    <p className="brew-card-meta">{pr.pr_url}</p>
                    <div className="brew-chip-row">
                      <span className="brew-chip">Bugs {summary.total}</span>
                      <span className="brew-chip">Open {summary.open}</span>
                      <span className="brew-chip">Needs Human {summary.needsHuman}</span>
                      <span className="brew-chip">Resolved {summary.resolved}</span>
                    </div>
                    <div className="brew-card-footer">Updated {formatDateTime(pr.updated_at)}</div>
                   </div>
                );
              })}
              {prs.length === 0 && !isLoadingPrs ? <div className="brew-empty-block">This project has no PR cards yet.</div> : null}
            </div>
          </section>
        </section>

        {selectedPr ? (
          <section className="brew-panel brew-bug-pool-panel">
            <div className="brew-panel-header">
              <div>
                <h3>Bug Pool</h3>
              </div>
            </div>

            <div className="brew-card-grid brew-bug-card-grid">
              {projectIssues.map((issue) => (
                <article key={issue.id} className="brew-card brew-bug-card">
                  <div className="brew-card-header">
                    <div>
                      <div className="brew-card-kicker">{issue.severity}</div>
                      <strong>{issue.title}</strong>
                    </div>
                    <span className="brew-chip">{issue.status}</span>
                  </div>
                  <p className="brew-card-meta">{issue.file_path}:{issue.start_line}-{issue.end_line}</p>
                  <div className="brew-chip-row">
                    <span className="brew-chip">PR #{issue.pr_number}</span>
                    <span className="brew-chip">Confidence {issue.confidence ?? '-'}</span>
                  </div>
                  <div className="brew-card-footer">Updated {formatDateTime(issue.updated_at)}</div>
                </article>
              ))}
              {projectIssues.length === 0 ? <div className="brew-empty-block">This PR has no bugs in pool.</div> : null}
            </div>
          </section>
        ) : null}

        {error ? <div className="brew-error-bar">{error}</div> : null}
      </main>

      {showCreateProject ? (
        <div className="brew-modal-overlay" onClick={() => setShowCreateProject(false)}>
          <div className="brew-modal" onClick={(e) => e.stopPropagation()}>
            <h3>Create Project</h3>
            <input
              className="brew-modal-input"
              placeholder="Project name"
              value={newProjectName}
              onChange={(e) => setNewProjectName(e.target.value)}
              autoFocus
              onKeyDown={(e) => e.key === 'Enter' && void handleCreateProject()}
            />
            <div className="brew-modal-actions">
              <button className="brew-btn-secondary" onClick={() => setShowCreateProject(false)}>取消</button>
              <button className="brew-btn-primary" onClick={() => void handleCreateProject()}>确定</button>
            </div>
          </div>
        </div>
      ) : null}

      {showCreatePr ? (
        <div className="brew-modal-overlay" onClick={() => setShowCreatePr(false)}>
          <div className="brew-modal" onClick={(e) => e.stopPropagation()}>
            <h3>添加 PR</h3>
            <input
              className="brew-modal-input"
              placeholder="PR URL"
              value={newPrUrl}
              onChange={(e) => setNewPrUrl(e.target.value)}
              autoFocus
              onKeyDown={(e) => e.key === 'Enter' && void handleCreatePr()}
            />
            <div className="brew-modal-actions">
              <button className="brew-btn-secondary" onClick={() => setShowCreatePr(false)}>取消</button>
              <button className="brew-btn-primary" onClick={() => void handleCreatePr()}>确定</button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function formatDateTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date);
}

function deriveProjectNameFromProject(project: ProjectSummary, prs: PullRequestSummary[]) {
  const relatedPr = prs.find((pr) => pr.project_id === project.id);
  return deriveProjectNameFromPrUrl(relatedPr?.pr_url) ?? project.project_name;
}

function deriveProjectNameFromPrUrl(prUrl?: string | null) {
  if (!prUrl) {
    return null;
  }

  try {
    const url = new URL(prUrl);
    const parts = url.pathname.split('/').filter(Boolean);
    if (parts.length >= 2) {
      return parts[1];
    }
  } catch {
    return null;
  }

  return null;
}

async function readApiError(response: Response) {
  try {
    const payload = (await response.json()) as { error?: string };
    return payload.error ?? `Request failed with status ${response.status}`;
  } catch {
    return `Request failed with status ${response.status}`;
  }
}

function toErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }
  return 'Unknown error';
}
