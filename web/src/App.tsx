import { useEffect, useMemo, useState, useRef } from 'react';
import DiffMatchPatch from 'diff-match-patch';

const API_BASE_URL = (import.meta.env.VITE_API_BASE_URL || `${window.location.origin}/api`).replace(/\/$/, '');

type ProjectSummary = {
  id: number;
  project_key: string;
  project_name: string;
  repo_url: string | null;
  repo_dir: string | null;
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
  description: string;
  suggestion: string;
  suggestion_code: string | null;
  original_code: string | null;
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
  const [newProjectRepoUrl, setNewProjectRepoUrl] = useState('');
  const [showCreatePr, setShowCreatePr] = useState(false);
  const [newPrUrl, setNewPrUrl] = useState('');
  const [pendingProjectKeyForPr, setPendingProjectKeyForPr] = useState<string | null>(null);
  const [openProjectMenuKey, setOpenProjectMenuKey] = useState<string | null>(null);
  const [hoveredReviewPrId, setHoveredReviewPrId] = useState<number | null>(null);
  const [selectedIssueId, setSelectedIssueId] = useState<number | null>(null);
  const [openIssueMenuId, setOpenIssueMenuId] = useState<number | null>(null);
  const [deleteConfirmIssueId, setDeleteConfirmIssueId] = useState<number | null>(null);

  const [workflowRuns, setWorkflowRuns] = useState<WorkflowRunSummary[]>([]);
  const [reviewRunningPrIds, setReviewRunningPrIds] = useState<number[]>([]);
  const [workflowRounds, setWorkflowRounds] = useState<WorkflowRoundSummary[]>([]);
  const [pendingReviewPrIds, setPendingReviewPrIds] = useState<number[]>([]);
  const [pendingIssueFixIds, setPendingIssueFixIds] = useState<number[]>([]);
  const [pendingFixAllPrIds, setPendingFixAllPrIds] = useState<number[]>([]);
  const prsRef = useRef<PullRequestSummary[]>([]);
  const projectIssuesRef = useRef<IssueSummary[]>([]);
  const isFixingRef = useRef(false);

  const selectedProject = useMemo(
    () => projects.find((project) => project.project_key === selectedProjectKey) ?? null,
    [projects, selectedProjectKey],
  );

  const selectedPr = useMemo(
    () => prs.find((pr) => pr.id === selectedPrId) ?? null,
    [prs, selectedPrId],
  );

  const selectedIssue = useMemo(
    () => projectIssues.find((issue) => issue.id === selectedIssueId) ?? null,
    [projectIssues, selectedIssueId],
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
      prsRef.current = data;

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
      projectIssuesRef.current = data;
      setPendingIssueFixIds((current) =>
        current.filter((id) => {
          const issue = data.find((i) => i.id === id);
          if (!issue) return false;
          return ['open', 'reopened', 'needs_human', 'claimed'].includes(issue.status);
        }),
      );
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
          .filter(([prId]) => {
            if (pendingFixAllPrIds.includes(prId)) return false;
            if (isFixingRef.current && prId === selectedPrId) return false;
            return true;
          })
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
        body: JSON.stringify({ project_name: newProjectName.trim(), repo_url: newProjectRepoUrl.trim() || null }),
      });
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }
      await loadProjects();
      setShowCreateProject(false);
      setNewProjectName('');
      setNewProjectRepoUrl('');
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

  async function handleFixIssue(issue: IssueSummary) {
    if (!selectedPr || !selectedProjectKey) {
      setError('Select a PR before fixing an issue');
      return;
    }

    setError(null);
    isFixingRef.current = true;
    setPendingIssueFixIds((current) => (current.includes(issue.id) ? current : [...current, issue.id]));
    try {
      const response = await fetch(`${API_BASE_URL}/issues/${issue.id}/fix`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      });
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }

      await loadProjectIssues(selectedProjectKey, selectedPr);
      await loadWorkflows(selectedProjectKey);
      await loadSelectedWorkflowRounds(selectedPr);
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      isFixingRef.current = false;
    }
  }

  async function handleFixAll(pr: PullRequestSummary) {
    if (!selectedProjectKey) {
      setError('Select a project before fixing issues');
      return;
    }

    setError(null);
    isFixingRef.current = true;
    setPendingFixAllPrIds((current) => (current.includes(pr.id) ? current : [...current, pr.id]));
    try {
      const response = await fetch(`${API_BASE_URL}/prs/${pr.id}/fix-all`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      });
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }

      await loadProjectIssues(selectedProjectKey, pr);
      await loadWorkflows(selectedProjectKey);
      await loadSelectedWorkflowRounds(pr);
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      isFixingRef.current = false;
      setPendingFixAllPrIds((current) => current.filter((item) => item !== pr.id));
    }
  }

  async function handleUpdateIssueStatus(issueId: number, newStatus: string) {
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/issues/${issueId}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ status: newStatus }),
      });
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }
      await loadProjectIssues(selectedProjectKey ?? '', selectedPr);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }

  async function handleDeleteIssue(issueId: number) {
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/issues/${issueId}`, {
        method: 'DELETE',
      });
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }
      await loadProjectIssues(selectedProjectKey ?? '', selectedPr);
    } catch (err) {
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

        <section className="brew-board-grid brew-prpool-layout">
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

          {selectedPr ? (
            <section className="brew-panel brew-bug-pool-panel">
              <div className="brew-panel-header">
                <div>
                  <h3>Bug Pool</h3>
                </div>
                <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                  <button
                    className="brew-clear-btn"
                    onClick={() => {
                      if (projectIssues.length === 0) return;
                      if (window.confirm(`Clear all ${projectIssues.length} bugs?`)) {
                        void Promise.all(projectIssues.map((issue) => handleDeleteIssue(issue.id)));
                      }
                    }}
                    disabled={projectIssues.length === 0 || pendingIssueFixIds.length > 0 || pendingFixAllPrIds.length > 0}
                  >
                    Clear
                  </button>
                  <button
                    className="brew-fix-action brew-fix-action-primary"
                    onClick={() => void handleFixAll(selectedPr)}
                    disabled={pendingFixAllPrIds.includes(selectedPr.id) || reviewRunningPrIds.includes(selectedPr.id) || pendingReviewPrIds.includes(selectedPr.id)}
                  >
                    {pendingFixAllPrIds.includes(selectedPr.id) ? <span className="brew-pr-run-spinner" aria-hidden="true" /> : 'Fix All'}
                  </button>
                </div>
              </div>

              <div className="brew-card-grid brew-bug-card-grid">
                {projectIssues.map((issue) => (
                  <article
                    key={issue.id}
                    className="brew-card brew-bug-card"
                    onClick={() => setSelectedIssueId(issue.id)}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault();
                        setSelectedIssueId(issue.id);
                      }
                    }}
                  >
                      <div className="brew-card-header">
                       <div>
                         <div className="brew-card-kicker">{issue.severity}</div>
                         <strong>{issue.title}</strong>
                       </div>
                       <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                         <div className="brew-issue-menu-wrap">
                           <button
                             className={`brew-status-dropdown brew-status-${issue.status}`}
                             onClick={(event) => {
                               event.stopPropagation();
                               setOpenIssueMenuId((current) => current === issue.id ? null : issue.id);
                             }}
                           >
                             {issue.status}
                             <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                               <polyline points="6 9 12 15 18 9"></polyline>
                             </svg>
                           </button>
                           {openIssueMenuId === issue.id ? (
                             <div className="brew-issue-menu">
                               {['open', 'resolved', 'needs_human', 'invalid'].map((statusOption) => (
                                <button
                                  key={statusOption}
                                  className="brew-issue-menu-item"
                                  onClick={(event) => {
                                    event.stopPropagation();
                                    void handleUpdateIssueStatus(issue.id, statusOption);
                                    setOpenIssueMenuId(null);
                                  }}
                                >
                                  {statusOption}
                                </button>
                              ))}
                             </div>
                           ) : null}
                         </div>
                          <button
                            className="brew-card-delete-btn"
                            onClick={(event) => {
                              event.stopPropagation();
                              setDeleteConfirmIssueId(issue.id);
                            }}
                            title="Delete issue"
                          >
                            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                             <polyline points="3 6 5 6 21 6"></polyline>
                             <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                           </svg>
                         </button>
                       </div>
                      </div>
                    <p className="brew-card-meta">{issue.file_path}:{issue.start_line}-{issue.end_line}</p>
                     <div className="brew-chip-row">
                       <span className="brew-chip">PR #{issue.pr_number}</span>
                       <span className="brew-chip">Confidence {issue.confidence ?? '-'}</span>
                     </div>
                     <div className="brew-card-footer">
                       <span>Updated {formatDateTime(issue.updated_at)}</span>
                       <button
                         className="brew-fix-action"
                         onClick={(event) => {
                           event.stopPropagation();
                           void handleFixIssue(issue);
                         }}
                        disabled={pendingIssueFixIds.includes(issue.id) || pendingFixAllPrIds.includes(selectedPr.id)}
                        >
                          {pendingIssueFixIds.includes(issue.id) ? <span className="brew-pr-run-spinner" aria-hidden="true" /> : 'Fix'}
                       </button>
                     </div>
                   </article>
                ))}
                {projectIssues.length === 0 ? <div className="brew-empty-block">This PR has no bugs in pool.</div> : null}
              </div>
            </section>
          ) : null}
        </section>

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
            <input
              className="brew-modal-input"
              placeholder="Repository URL (e.g. https://github.com/owner/repo)"
              value={newProjectRepoUrl}
              onChange={(e) => setNewProjectRepoUrl(e.target.value)}
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

      {selectedIssue ? (
        <div className="brew-modal-overlay" onClick={() => setSelectedIssueId(null)}>
          <div className="brew-modal brew-modal-wide" onClick={(e) => e.stopPropagation()}>
            {/* Header */}
            <div className="brew-issue-header">
              <div className="brew-issue-header-title">
                <span className={`brew-issue-severity brew-issue-severity-${selectedIssue.severity}`}>{selectedIssue.severity}</span>
                <h3>{selectedIssue.title}</h3>
              </div>
              <div className="brew-issue-header-meta">
                <span className={`brew-status-pill brew-status-${selectedIssue.status}`}>{selectedIssue.status}</span>
                {selectedIssue.confidence !== null ? (
                  <span className="brew-issue-confidence">Confidence {selectedIssue.confidence}%</span>
                ) : null}
              </div>
            </div>

            {/* Location */}
            <div className="brew-issue-location">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"></path>
                <polyline points="13 2 13 9 20 9"></polyline>
              </svg>
              <code>{selectedIssue.file_path}</code>
              <span className="brew-issue-line">Line {selectedIssue.start_line}{selectedIssue.end_line !== selectedIssue.start_line ? ` - ${selectedIssue.end_line}` : ''}</span>
            </div>

            {/* Description */}
            <div className="brew-issue-block">
              <div className="brew-issue-block-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="12" cy="12" r="10"></circle>
                  <line x1="12" y1="16" x2="12" y2="12"></line>
                  <line x1="12" y1="8" x2="12.01" y2="8"></line>
                </svg>
                Description
              </div>
              <div className="brew-issue-block-content">{selectedIssue.description}</div>
            </div>

            {/* Suggestion */}
            <div className="brew-issue-block">
              <div className="brew-issue-block-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M12 20h9"></path>
                  <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"></path>
                </svg>
                Suggestion
              </div>
              <div className="brew-issue-block-content">{selectedIssue.suggestion}</div>
            </div>

            {/* Suggested Code Diff */}
            {selectedIssue.suggestion_code ? (
              <div className="brew-issue-block">
                <div className="brew-issue-block-title">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="16 18 22 12 16 6"></polyline>
                    <polyline points="8 6 2 12 8 18"></polyline>
                  </svg>
                  Suggested Fix
                </div>
                <CodeDiffViewer
                  oldCode={selectedIssue.original_code || ''}
                  newCode={selectedIssue.suggestion_code}
                  startLine={selectedIssue.start_line}
                />
              </div>
            ) : null}

            {/* Footer */}
            <div className="brew-issue-footer">
              <span>PR #{selectedIssue.pr_number} · {selectedIssue.platform}</span>
              <span>Updated {formatDateTime(selectedIssue.updated_at)}</span>
            </div>

            <div className="brew-modal-actions">
              <button
                className="brew-btn-primary"
                onClick={() => void handleFixIssue(selectedIssue)}
                disabled={pendingIssueFixIds.includes(selectedIssue.id) || pendingFixAllPrIds.includes(selectedPr.id)}
              >
                {pendingIssueFixIds.includes(selectedIssue.id) ? <span className="brew-pr-run-spinner" aria-hidden="true" /> : 'Fix'}
              </button>
              <button className="brew-btn-secondary" onClick={() => setSelectedIssueId(null)}>关闭</button>
            </div>
          </div>
        </div>
      ) : null}

      {deleteConfirmIssueId !== null ? (
        <div className="brew-modal-overlay" onClick={() => setDeleteConfirmIssueId(null)}>
          <div className="brew-modal" onClick={(e) => e.stopPropagation()}>
            <h3>确认删除</h3>
            <p style={{ margin: 0, color: 'var(--text-secondary)', fontSize: 14 }}>
              确定要删除这个 bug 吗？此操作不可撤销。
            </p>
            <div className="brew-modal-actions">
              <button className="brew-btn-secondary" onClick={() => setDeleteConfirmIssueId(null)}>取消</button>
              <button
                className="brew-btn-primary"
                style={{ backgroundColor: '#e53e3e' }}
                onClick={() => {
                  const id = deleteConfirmIssueId;
                  setDeleteConfirmIssueId(null);
                  void handleDeleteIssue(id);
                }}
              >
                删除
              </button>
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

// ---------------------------------------------------------------------------
// Code Diff Viewer Component
// ---------------------------------------------------------------------------

function CodeDiffViewer({
  oldCode,
  newCode,
  startLine = 1,
}: {
  oldCode: string;
  newCode: string;
  startLine?: number;
}) {
  const dmp = new DiffMatchPatch();

  const diffLines = useMemo(() => {
    const diffs = dmp.diff_main(oldCode, newCode);
    dmp.diff_cleanupSemantic(diffs);

    const result: Array<{
      type: 'equal' | 'insert' | 'delete';
      oldLineNum: number | null;
      newLineNum: number | null;
      text: string;
    }> = [];

    let oldLineNum = startLine;
    let newLineNum = startLine;

    for (const [op, text] of diffs) {
      const lines = text.split('\n');
      // diff-match-patch does not include trailing newline in the chunk,
      // so if text ends with newline the last element after split is ''.
      const hasTrailingNewline = text.endsWith('\n');
      const lineCount = hasTrailingNewline ? lines.length - 1 : lines.length;

      for (let i = 0; i < lineCount; i++) {
        const lineText = lines[i];
        if (op === 0) {
          // equal
          result.push({
            type: 'equal',
            oldLineNum,
            newLineNum,
            text: lineText,
          });
          oldLineNum++;
          newLineNum++;
        } else if (op === -1) {
          // delete
          result.push({
            type: 'delete',
            oldLineNum,
            newLineNum: null,
            text: lineText,
          });
          oldLineNum++;
        } else if (op === 1) {
          // insert
          result.push({
            type: 'insert',
            oldLineNum: null,
            newLineNum,
            text: lineText,
          });
          newLineNum++;
        }
      }
    }

    return result;
  }, [oldCode, newCode, startLine]);

  return (
    <div className="diff-viewer">
      {diffLines.map((line, idx) => (
        <div key={idx} className={`diff-line diff-${line.type}`}>
          <span className="diff-gutter diff-gutter-old">
            {line.oldLineNum ?? ''}
          </span>
          <span className="diff-gutter diff-gutter-new">
            {line.newLineNum ?? ''}
          </span>
          <span className="diff-marker">
            {line.type === 'equal' ? ' ' : line.type === 'insert' ? '+' : '-'}
          </span>
          <span className="diff-content">{line.text}</span>
        </div>
      ))}
    </div>
  );
}
