import { useEffect, useMemo, useState } from 'react';

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

  const selectedProject = useMemo(
    () => projects.find((project) => project.project_key === selectedProjectKey) ?? null,
    [projects, selectedProjectKey],
  );

  const selectedPr = useMemo(
    () => prs.find((pr) => pr.id === selectedPrId) ?? null,
    [prs, selectedPrId],
  );

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
      return;
    }

    void loadPrs(selectedProjectKey);
    void loadProjectIssues(selectedProjectKey);
  }, [selectedProjectKey]);

  async function loadProjects() {
    setIsLoadingProjects(true);
    setError(null);
    try {
      const response = await fetch('/api/projects');
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
      const response = await fetch(`/api/prs?project_key=${encodeURIComponent(projectKey)}`);
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

  async function loadProjectIssues(projectKey: string) {
    try {
      const response = await fetch(`/api/issues?project_key=${encodeURIComponent(projectKey)}`);
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

  function openSelectedPr() {
    if (!selectedPr) {
      return;
    }
    window.open(selectedPr.pr_url, '_blank', 'noopener,noreferrer');
  }

  function handleAddPr() {
    setActiveNav('prpool');
    setError('Add PR 入口已预留，后续接入创建表单或导入流程。');
  }

  return (
    <div className="brew-shell">
      <aside className="brew-sidebar">
        <div className="brew-brand">
          <div className="brew-brand-mark">M</div>
          <div>
            <div className="brew-brand-kicker">FixAgent Console</div>
            <h1>PR Review Board</h1>
          </div>
        </div>

        <nav className="brew-nav">
          <button className={isProjectMenuOpen ? 'brew-nav-item brew-nav-item-active' : 'brew-nav-item'} onClick={() => setIsProjectMenuOpen((value) => !value)}>
            <span>Project</span>
            <span className="brew-nav-caret">{isProjectMenuOpen ? '-' : '+'}</span>
          </button>
        </nav>

        {isProjectMenuOpen ? <section className="brew-sidebar-section brew-sidebar-subsection">
          <div className="brew-section-header">
            <span>Project List</span>
            <button className="brew-link-button" onClick={() => void loadProjects()} disabled={isLoadingProjects}>
              Refresh
            </button>
          </div>

          <div className="brew-project-list">
            {projects.map((project) => (
              <button
                key={project.project_key}
                className={project.project_key === selectedProjectKey ? 'brew-project-chip brew-project-chip-active' : 'brew-project-chip'}
                onClick={() => {
                  setSelectedProjectKey(project.project_key);
                }}
              >
                <div className="brew-project-chip-title">{project.project_name}</div>
                <div className="brew-project-chip-meta">{project.project_key}</div>
              </button>
            ))}
            {projects.length === 0 && !isLoadingProjects ? <div className="brew-empty-mini">No projects yet.</div> : null}
          </div>
        </section> : null}
      </aside>

      <main className="brew-main">
        <header className="brew-topbar">
          <div>
              <div className="brew-topbar-kicker">Project → PRPool</div>
              <div className="brew-topbar-title-row">
                <h2>{inferredProjectName}</h2>
                <span className="brew-tag">English Mild Ale Palette</span>
              </div>
          </div>

          <div className="brew-topbar-actions">
            <button className="brew-ghost-button" onClick={() => {
              void loadProjects();
              if (selectedProjectKey) {
                void loadPrs(selectedProjectKey);
                void loadProjectIssues(selectedProjectKey);
              }
            }}>
              Refresh
            </button>
            <button className="brew-ghost-button" onClick={openSelectedPr} disabled={!selectedPr}>
              Open PR
            </button>
          </div>
        </header>

        <section className="brew-hero-card">
          <div className="brew-hero-copy">
            <div className="brew-hero-badge">PR Pool</div>
            <h3>A clean PR board with only pull request cards.</h3>
            <p>
              {selectedProject
                ? `${deriveProjectNameFromProject(selectedProject, prs)} currently has ${projectMetrics.prs} PR cards and ${projectMetrics.bugs} total review bugs.`
                : 'Choose a project to unlock the PR pool.'}
            </p>
          </div>

          <div className="brew-hero-stats">
            <div className="brew-stat-block">
              <span>Projects</span>
              <strong>{String(projects.length).padStart(2, '0')}</strong>
            </div>
            <div className="brew-stat-block">
              <span>PRs</span>
              <strong>{String(projectMetrics.prs).padStart(2, '0')}</strong>
            </div>
            <div className="brew-stat-block">
              <span>Open Bugs</span>
              <strong>{String(projectMetrics.open).padStart(2, '0')}</strong>
            </div>
          </div>
        </section>

        <section className="brew-prpool-layout">
          <div className="brew-prpool-toolbar">
            <div className="brew-panel-note">Only PR cards here.</div>
            <button className="brew-add-button" onClick={handleAddPr}>Add PR</button>
          </div>

          <section className="brew-panel brew-prpool-panel">
            <div className="brew-panel-header">
              <div>
                <div className="brew-panel-kicker">PR Cards</div>
                <h3>PR Pool</h3>
              </div>
              <div className="brew-panel-note">Compact PR card board.</div>
            </div>

            <div className="brew-card-grid brew-pr-card-grid">
              {prs.map((pr) => {
                const summary = prIssueSummaryMap.get(pr.pr_number) ?? { total: 0, open: 0, needsHuman: 0, resolved: 0 };
                return (
                  <button
                    key={pr.id}
                    className={pr.id === selectedPrId ? 'brew-card brew-card-active brew-pr-card' : 'brew-card brew-pr-card'}
                    onClick={() => setSelectedPrId(pr.id)}
                  >
                    <div className="brew-card-header">
                      <div>
                        <div className="brew-card-kicker">{pr.platform}</div>
                        <strong>PR #{pr.pr_number}</strong>
                      </div>
                      <span className="brew-card-mark brew-card-mark-pr">R</span>
                    </div>
                    <p className="brew-card-meta">{pr.pr_url}</p>
                    <div className="brew-chip-row">
                      <span className="brew-chip">Bugs {summary.total}</span>
                      <span className="brew-chip">Open {summary.open}</span>
                      <span className="brew-chip">Needs Human {summary.needsHuman}</span>
                      <span className="brew-chip">Resolved {summary.resolved}</span>
                    </div>
                    <div className="brew-card-footer">Updated {formatDateTime(pr.updated_at)}</div>
                  </button>
                );
              })}
              {prs.length === 0 && !isLoadingPrs ? <div className="brew-empty-block">This project has no PR cards yet.</div> : null}
            </div>
          </section>
        </section>

        {error ? <div className="brew-error-bar">{error}</div> : null}
      </main>
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
