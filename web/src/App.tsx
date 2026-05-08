import { useEffect, useMemo, useState } from 'react';

type NavKey = 'workspace' | 'workflow' | 'projects' | 'settings';
type DetailTab = 'overview' | 'rounds' | 'issues' | 'status';

type IssueSummary = {
  id: number;
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

type WorkflowRunDetail = {
  workflow: WorkflowRunSummary;
  rounds: WorkflowRoundSummary[];
};

type FormState = {
  repoDir: string;
  projectKey: string;
  projectName: string;
  prUrl: string;
  claimedBy: string;
  maxRounds: string;
  dryRun: boolean;
};

type LaunchState = {
  projectName: string;
  prUrl: string;
  startedAt: string;
};

const initialForm: FormState = {
  repoDir: '/workspace',
  projectKey: '',
  projectName: '',
  prUrl: '',
  claimedBy: 'frontend',
  maxRounds: '5',
  dryRun: true,
};

export default function App() {
  const [form, setForm] = useState<FormState>(initialForm);
  const [workflows, setWorkflows] = useState<WorkflowRunSummary[]>([]);
  const [selectedWorkflowId, setSelectedWorkflowId] = useState<number | null>(null);
  const [workflowDetail, setWorkflowDetail] = useState<WorkflowRunDetail | null>(null);
  const [issues, setIssues] = useState<IssueSummary[]>([]);
  const [activeNav, setActiveNav] = useState<NavKey>('workspace');
  const [activeTab, setActiveTab] = useState<DetailTab>('overview');
  const [autoRefresh, setAutoRefresh] = useState(false);
  const [launchState, setLaunchState] = useState<LaunchState | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isLoadingWorkflows, setIsLoadingWorkflows] = useState(true);
  const [isLoadingDetail, setIsLoadingDetail] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void loadWorkflows();
  }, []);

  useEffect(() => {
    if (selectedWorkflowId == null) {
      setWorkflowDetail(null);
      setIssues([]);
      return;
    }
    void loadWorkflowDetail(selectedWorkflowId);
  }, [selectedWorkflowId]);

  useEffect(() => {
    if (!autoRefresh) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void loadWorkflows();
      if (selectedWorkflowId != null) {
        void loadWorkflowDetail(selectedWorkflowId);
      }
    }, 10000);

    return () => window.clearInterval(intervalId);
  }, [autoRefresh, selectedWorkflowId]);

  const selectedWorkflow = workflowDetail?.workflow ?? null;

  const currentStage = useMemo(() => {
    if (launchState && !workflowDetail) {
      return '正在创建 workflow 任务';
    }

    if (!workflowDetail) {
      return '等待启动';
    }

    if (workflowDetail.workflow.status !== 'running') {
      return workflowDetail.workflow.summary ?? workflowDetail.workflow.status;
    }

    const runningRound = [...workflowDetail.rounds]
      .reverse()
      .find((round) => round.status === 'running');

    if (runningRound?.summary) {
      return runningRound.summary;
    }

    return workflowDetail.workflow.summary ?? 'Workflow 正在运行';
  }, [launchState, workflowDetail]);

  const importantIssueCount = issues.filter((issue) => ['open', 'reopened', 'needs_human'].includes(issue.status)).length;

  const issueMetrics = useMemo(() => {
    return {
      total: issues.length,
      open: issues.filter((issue) => issue.status === 'open').length,
      reopened: issues.filter((issue) => issue.status === 'reopened').length,
      needsHuman: issues.filter((issue) => issue.status === 'needs_human').length,
      resolved: issues.filter((issue) => issue.status === 'resolved').length,
    };
  }, [issues]);

  const metrics = useMemo(() => {
    if (!workflowDetail) {
      return [
        { label: '状态', value: '待执行' },
        { label: '轮次', value: '0' },
        { label: '错误池', value: '0' },
      ];
    }

    return [
      { label: '状态', value: workflowDetail.workflow.status },
      { label: '轮次', value: String(workflowDetail.rounds.length) },
      { label: '错误池', value: String(issueMetrics.total) },
    ];
  }, [issueMetrics.total, workflowDetail]);

  const statusRows = useMemo(() => {
    if (!workflowDetail) {
      return [];
    }

      return [
        { label: 'Workflow 状态', value: workflowDetail.workflow.status },
        { label: '当前阶段', value: currentStage },
        { label: '已完成轮次', value: String(workflowDetail.rounds.filter((round) => round.status === 'completed').length) },
        { label: '待处理问题', value: String(importantIssueCount) },
      ];
  }, [currentStage, importantIssueCount, workflowDetail]);

  const visibleWorkflows = useMemo(() => {
    switch (activeNav) {
      case 'workflow':
        return workflows;
      case 'projects':
        return workflows.filter((workflow, index, list) => list.findIndex((item) => item.project_key === workflow.project_key) === index);
      case 'settings':
        return [];
      case 'workspace':
      default:
        return workflows.slice(0, 6);
    }
  }, [activeNav, workflows]);

  const heroStats = useMemo(() => {
    const totalRuns = workflows.length;
    const runningRuns = workflows.filter((workflow) => workflow.status === 'running').length;
    const completedRuns = workflows.filter((workflow) => workflow.status === 'completed').length;

    return [
      { label: 'Total Runs', value: String(totalRuns).padStart(2, '0') },
      { label: 'Running', value: String(runningRuns).padStart(2, '0') },
      { label: 'Completed', value: String(completedRuns).padStart(2, '0') },
    ];
  }, [workflows]);

  async function loadWorkflows() {
    setIsLoadingWorkflows(true);
    setError(null);
    try {
      const response = await fetch('/api/workflows');
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }
      const data = (await response.json()) as WorkflowRunSummary[];
      setWorkflows(data);
      const preferredWorkflow = pickPreferredWorkflow(data, selectedWorkflowId);
      if (preferredWorkflow) {
        setSelectedWorkflowId(preferredWorkflow.id);
      } else if (data.length === 0) {
        setSelectedWorkflowId(null);
      }
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setIsLoadingWorkflows(false);
    }
  }

  async function loadWorkflowDetail(workflowId: number) {
    setIsLoadingDetail(true);
    setError(null);
    try {
      const response = await fetch(`/api/workflows/${workflowId}`);
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }
      const data = (await response.json()) as WorkflowRunDetail;
      setWorkflowDetail(data);
      await loadIssuesForWorkflow(data.workflow);
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setIsLoadingDetail(false);
    }
  }

  async function loadIssuesForWorkflow(workflow: WorkflowRunSummary) {
    try {
      const response = await fetch(
        `/api/issues?project_key=${encodeURIComponent(workflow.project_key)}&platform=${encodeURIComponent(workflow.platform)}&pr_number=${workflow.pr_number}`,
      );
      if (!response.ok) {
        throw new Error(await readApiError(response));
      }
      const data = (await response.json()) as IssueSummary[];
      setIssues(data);
    } catch (err) {
      setIssues([]);
      throw err;
    }
  }

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true);
    setError(null);
    setActiveNav('workflow');
    setActiveTab('rounds');
    setAutoRefresh(true);
    setLaunchState({
      projectName: form.projectName || form.projectKey || '新 Workflow',
      prUrl: form.prUrl,
      startedAt: new Date().toISOString(),
    });
    try {
      const response = await fetch('/api/workflows', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          repo_dir: form.repoDir,
          project_key: form.projectKey,
          project_name: form.projectName,
          pr_url: form.prUrl,
          claimed_by: form.claimedBy,
          max_rounds: Number(form.maxRounds),
          dry_run: form.dryRun,
        }),
      });

      if (!response.ok) {
        throw new Error(await readApiError(response));
      }

      const result = (await response.json()) as { workflow_run_id: number };
      await loadWorkflows();
      setSelectedWorkflowId(result.workflow_run_id);
    } catch (err) {
      setError(toErrorMessage(err));
      setLaunchState(null);
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <div className="console-shell">
      <aside className="sidebar">
        <div className="sidebar-brand">
          <div className="sidebar-logo">M</div>
          <div>
            <div className="eyebrow">Workflow Intelligence</div>
            <h1>FixAgent</h1>
          </div>
        </div>

        <nav className="sidebar-nav">
          <button className={activeNav === 'workspace' ? 'nav-item nav-item-active' : 'nav-item'} onClick={() => setActiveNav('workspace')}>工作台</button>
          <button className={activeNav === 'workflow' ? 'nav-item nav-item-active' : 'nav-item'} onClick={() => setActiveNav('workflow')}>Workflow</button>
          <button className={activeNav === 'projects' ? 'nav-item nav-item-active' : 'nav-item'} onClick={() => setActiveNav('projects')}>项目</button>
          <button className={activeNav === 'settings' ? 'nav-item nav-item-active' : 'nav-item'} onClick={() => setActiveNav('settings')}>设置</button>
        </nav>

        <section className="sidebar-section">
          <div className="section-mini-header">
            <span>{activeNav === 'projects' ? '项目列表' : activeNav === 'workflow' ? '全部运行' : activeNav === 'settings' ? '控制选项' : '最近运行'}</span>
            <button className="text-button" onClick={() => void loadWorkflows()} disabled={isLoadingWorkflows}>
              刷新
            </button>
          </div>

          <div className="sidebar-run-list">
            {activeNav === 'settings' ? (
              <div className="mini-settings-stack">
                <button className={autoRefresh ? 'run-chip run-chip-active' : 'run-chip'} onClick={() => setAutoRefresh((value) => !value)}>
                  <div className="run-chip-title">自动刷新</div>
                  <div className="run-chip-meta">{autoRefresh ? '已开启，每 10 秒刷新' : '点击开启'}</div>
                </button>
                <button
                  className="run-chip"
                  onClick={() => {
                    setForm(initialForm);
                    setError(null);
                    setActiveNav('workspace');
                  }}
                >
                  <div className="run-chip-title">重置表单</div>
                  <div className="run-chip-meta">恢复默认启动参数</div>
                </button>
              </div>
            ) : (
              visibleWorkflows.map((workflow) => (
                <button
                  key={workflow.id}
                  className={workflow.id === selectedWorkflowId ? 'run-chip run-chip-active' : 'run-chip'}
                  onClick={() => setSelectedWorkflowId(workflow.id)}
                >
                  <div className="run-chip-title">{workflow.project_name || workflow.project_key}</div>
                  <div className="run-chip-meta">{activeNav === 'projects' ? workflow.project_key : `PR #${workflow.pr_number}`}</div>
                </button>
              ))
            )}
            {visibleWorkflows.length === 0 && !isLoadingWorkflows && activeNav !== 'settings' ? <div className="mini-empty">暂无可展示内容</div> : null}
          </div>
        </section>
      </aside>

      <div className="content-shell">
        <header className="topbar">
          <div>
            <div className="eyebrow">Autonomous Review Workspace</div>
            <div className="topbar-title-row">
              <h2>Workflow Console</h2>
              <span className="soft-badge">Review, Fix, Verify</span>
            </div>
          </div>

          <div className="topbar-actions">
            <button
              className="ghost-button"
              onClick={() => {
                document.getElementById('launch-form')?.scrollIntoView({ behavior: 'smooth', block: 'start' });
                setActiveNav('workspace');
              }}
            >
              新建流程
            </button>
            <button className={autoRefresh ? 'ghost-button ghost-button-active' : 'ghost-button'} onClick={() => setAutoRefresh((value) => !value)}>
              {autoRefresh ? '自动刷新开' : '自动刷新关'}
            </button>
            <button className="ghost-button" onClick={() => void loadWorkflows()} disabled={isLoadingWorkflows}>
              {isLoadingWorkflows ? '刷新中' : '刷新'}
            </button>
          </div>
        </header>

        <section className="project-banner">
            <div className="project-banner-main">
              <div className="project-avatar">F</div>
              <div>
                <h3>{selectedWorkflow?.project_name ?? launchState?.projectName ?? 'FixAgent Workflow'}</h3>
                <p>{selectedWorkflow?.pr_url ?? launchState?.prUrl ?? '输入 PR 后自动推进 review、fix、verify，直到 workflow 收敛。界面会持续展示关键轮次和当前处理状态。'}</p>
              </div>
            </div>

          <div className="project-banner-actions">
            <div className="hero-stats-row">
              {heroStats.map((item) => (
                <div key={item.label} className="hero-stat-card">
                  <span>{item.label}</span>
                  <strong>{item.value}</strong>
                </div>
              ))}
            </div>
            <div className="banner-status">
              <span className="status-dot" />
              {selectedWorkflow?.status ?? (isSubmitting ? '正在创建 workflow' : '等待创建')}
            </div>
            <button className="primary-button" type="submit" form="launch-form" disabled={isSubmitting}>
              {isSubmitting ? '启动中...' : '启动 AI'}
            </button>
          </div>
        </section>

        <div className="dashboard-grid">
          <main className="main-stage">
            <section className="panel form-panel">
              <div className="panel-header">
                <div>
                  <div className="panel-kicker">创建流程</div>
                  <h3>提交新的 PR Workflow</h3>
                </div>
                <div className="panel-header-note">系统会自动串联 review、fix、verify</div>
              </div>

              <div className="form-intro-grid">
                <div className="form-intro-card">
                  <span>适用场景</span>
                  <strong>针对单个 PR/MR 发起收敛式修复流程</strong>
                </div>
                <div className="form-intro-card">
                  <span>推荐方式</span>
                  <strong>先用 Dry Run 验证，再切真实修复</strong>
                </div>
              </div>

              <form className="launch-form" id="launch-form" onSubmit={handleSubmit}>
                <div className="field-grid field-grid-double">
                  <Field label="Repository">
                    <input
                      value={form.repoDir}
                      onChange={(event) => setForm({ ...form, repoDir: event.target.value })}
                      placeholder="/workspace"
                    />
                  </Field>
                  <Field label="Claimed By">
                    <input
                      value={form.claimedBy}
                      onChange={(event) => setForm({ ...form, claimedBy: event.target.value })}
                    />
                  </Field>
                </div>

                <div className="field-grid field-grid-double">
                  <Field label="Project Key">
                    <input
                      value={form.projectKey}
                      onChange={(event) => setForm({ ...form, projectKey: event.target.value })}
                      placeholder="github.com/acme/demo"
                    />
                  </Field>
                  <Field label="Project Name">
                    <input
                      value={form.projectName}
                      onChange={(event) => setForm({ ...form, projectName: event.target.value })}
                      placeholder="demo"
                    />
                  </Field>
                </div>

                <Field label="PR URL">
                  <input
                    value={form.prUrl}
                    onChange={(event) => setForm({ ...form, prUrl: event.target.value })}
                    placeholder="https://github.com/acme/demo/pull/123"
                  />
                </Field>

                <div className="field-grid field-grid-compact">
                  <Field label="Max Rounds">
                    <input
                      value={form.maxRounds}
                      onChange={(event) => setForm({ ...form, maxRounds: event.target.value })}
                      inputMode="numeric"
                    />
                  </Field>
                  <label className="toggle-field">
                    <span>Dry Run</span>
                    <button
                      type="button"
                      className={form.dryRun ? 'toggle toggle-active' : 'toggle'}
                      onClick={() => setForm({ ...form, dryRun: !form.dryRun })}
                    >
                      <span />
                    </button>
                  </label>
                </div>
              </form>

              {error ? <div className="inline-error">{error}</div> : null}
            </section>

            <section className="panel detail-panel">
                <div className="panel-header panel-header-with-tabs">
                  <div>
                    <div className="panel-kicker">执行详情</div>
                    <h3>Workflow Detail</h3>
                  </div>

                  <div className="tab-strip">
                    <button className={activeTab === 'overview' ? 'tab-item tab-item-active' : 'tab-item'} onClick={() => setActiveTab('overview')}>概览</button>
                    <button className={activeTab === 'rounds' ? 'tab-item tab-item-active' : 'tab-item'} onClick={() => setActiveTab('rounds')}>轮次</button>
                    <button className={activeTab === 'issues' ? 'tab-item tab-item-active' : 'tab-item'} onClick={() => setActiveTab('issues')}>错误池</button>
                    <button className={activeTab === 'status' ? 'tab-item tab-item-active' : 'tab-item'} onClick={() => setActiveTab('status')}>状态</button>
                  </div>
                </div>

                <div className="metric-row">
                  {metrics.map((metric) => (
                  <MetricCard key={metric.label} label={metric.label} value={metric.value} />
                ))}
              </div>

              {isLoadingDetail ? <div className="empty-state">正在加载 workflow 详情...</div> : null}

              {!isLoadingDetail && isSubmitting && !workflowDetail ? (
                <div className="empty-state empty-state-large">
                  <div className="empty-illustration">...</div>
                  <h4>正在启动 Workflow</h4>
                  <p>后端当前是同步执行模式。请求已经发出，页面会在拿到 `workflow_run_id` 后自动切换到最新任务。</p>
                </div>
              ) : null}

              {!isLoadingDetail && workflowDetail && activeTab === 'overview' ? (
                <div className="overview-panel-grid">
                  <div className="summary-card">
                    <div className="summary-card-label">摘要</div>
                    <strong>{workflowDetail.workflow.summary ?? '当前 workflow 已进入自动处理流程，等待更多执行摘要。'}</strong>
                    <p>系统会围绕当前 PR 持续执行 review、fix 与 verification，并将结果沉淀为可追踪轮次。</p>
                  </div>
                  <div className="summary-card summary-card-muted">
                    <div className="summary-card-label">当前阶段</div>
                    <strong>{currentStage}</strong>
                    <p>Started at {formatDateTime(workflowDetail.workflow.started_at)}{workflowDetail.workflow.completed_at ? ` · Completed at ${formatDateTime(workflowDetail.workflow.completed_at)}` : ''}</p>
                  </div>
                </div>
              ) : null}

              {!isLoadingDetail && workflowDetail && activeTab === 'rounds' ? (
                <div className="timeline-list">
                  {workflowDetail.rounds.length === 0 ? (
                    <div className="empty-state">暂无轮次数据。</div>
                  ) : (
                    workflowDetail.rounds.map((round) => (
                      <article key={round.id} className="timeline-card">
                        <div className="timeline-marker">{round.round_number}</div>
                        <div className="timeline-content">
                          <div className="timeline-title-row">
                            <strong>Round {round.round_number}</strong>
                            <span className={`status-pill status-${round.status}`}>{round.status}</span>
                          </div>
                          <div className="timeline-chip-row">
                            <span className="timeline-chip">Issue {round.issue_id ?? '-'}</span>
                            <span className="timeline-chip">Fix {round.fix_run_id ?? '-'}</span>
                            <span className="timeline-chip">Verify {round.verification_id ?? '-'}</span>
                          </div>
                          <div className="timeline-meta">
                            Started {formatDateTime(round.started_at)}{round.completed_at ? ` · Completed ${formatDateTime(round.completed_at)}` : ''}
                          </div>
                          <p>{round.summary ?? 'No summary recorded.'}</p>
                          <div className="timeline-footer">{round.stop_reason ?? '继续下一轮'}</div>
                        </div>
                      </article>
                    ))
                  )}
                </div>
              ) : null}

              {!isLoadingDetail && workflowDetail && activeTab === 'issues' ? (
                <div className="timeline-list">
                  {issues.length === 0 ? (
                    <div className="empty-state">当前 PR 还没有错误池数据。</div>
                  ) : (
                    issues.map((issue) => (
                      <article key={issue.id} className="timeline-card">
                        <div className="timeline-marker">{issue.severity.slice(0, 1).toUpperCase()}</div>
                        <div className="timeline-content">
                          <div className="timeline-title-row">
                            <strong>{issue.title}</strong>
                            <span className={`status-pill status-${issue.status}`}>{issue.status}</span>
                          </div>
                          <div className="timeline-chip-row">
                            <span className="timeline-chip">{issue.severity}</span>
                            <span className="timeline-chip">{issue.file_path}:{issue.start_line}</span>
                            <span className="timeline-chip">置信度 {issue.confidence ?? '-'}</span>
                          </div>
                          <div className="timeline-meta">
                            Updated {formatDateTime(issue.updated_at)}
                          </div>
                        </div>
                      </article>
                    ))
                  )}
                </div>
              ) : null}

              {!isLoadingDetail && workflowDetail && activeTab === 'status' ? (
                <div className="status-list">
                  {statusRows.map((row) => (
                    <div key={row.label} className="status-row-card">
                      <span>{row.label}</span>
                      <strong>{row.value}</strong>
                    </div>
                  ))}
                </div>
              ) : null}

              {!isLoadingDetail && !workflowDetail && !isSubmitting ? (
                <div className="empty-state empty-state-large">
                  <div className="empty-illustration">+</div>
                  <h4>暂无内容</h4>
                  <p>可以点击上方的“启动 AI”，从一个 PR 开始创建自动修复流程。</p>
                </div>
              ) : null}
            </section>
          </main>

          <aside className="right-rail">
            <section className="panel rail-panel">
              <div className="panel-header">
                <div>
                  <div className="panel-kicker">当前运行</div>
                  <h3>Overview</h3>
                </div>
              </div>

              {selectedWorkflow ? (
                <>
                  <div className="overview-highlight-card">
                    <span>当前焦点</span>
                    <strong>{selectedWorkflow.project_name}</strong>
                    <p>{currentStage}</p>
                  </div>
                  <div className="overview-stack">
                    <OverviewRow label="项目" value={selectedWorkflow.project_name} />
                    <OverviewRow label="平台" value={selectedWorkflow.platform} />
                    <OverviewRow label="PR" value={`#${selectedWorkflow.pr_number}`} />
                    <OverviewRow label="状态" value={selectedWorkflow.status} />
                    <OverviewRow label="错误池" value={String(issueMetrics.total)} />
                    <OverviewRow label="最大轮次" value={String(selectedWorkflow.max_rounds)} />
                  </div>
                  <div className="status-list compact-status-list">
                    <div className="status-row-card">
                      <span>Open</span>
                      <strong>{issueMetrics.open}</strong>
                    </div>
                    <div className="status-row-card">
                      <span>Reopened</span>
                      <strong>{issueMetrics.reopened}</strong>
                    </div>
                    <div className="status-row-card">
                      <span>Needs Human</span>
                      <strong>{issueMetrics.needsHuman}</strong>
                    </div>
                    <div className="status-row-card">
                      <span>Resolved</span>
                      <strong>{issueMetrics.resolved}</strong>
                    </div>
                  </div>
                </>
              ) : (
                <div className="mini-empty">选择一个 workflow 查看概览。</div>
              )}
            </section>

            <section className="panel rail-panel">
              <div className="panel-header">
                <div>
                  <div className="panel-kicker">运行历史</div>
                  <h3>Recent Runs</h3>
                </div>
              </div>

              <div className="history-list">
                {workflows.length === 0 && !isLoadingWorkflows ? <div className="mini-empty">暂无运行记录</div> : null}
                {workflows.map((workflow) => (
                  <button
                    key={workflow.id}
                    className={workflow.id === selectedWorkflowId ? 'history-card history-card-active' : 'history-card'}
                    onClick={() => {
                      setSelectedWorkflowId(workflow.id);
                      setActiveTab('overview');
                    }}
                  >
                    <div className="history-card-top">
                      <strong>{workflow.project_name || workflow.project_key}</strong>
                      <span className={`status-pill status-${workflow.status}`}>{workflow.status}</span>
                    </div>
                    <div className="history-card-meta">{workflow.platform} · PR #{workflow.pr_number}</div>
                    <div className="history-card-summary">{workflow.summary ?? workflow.pr_url}</div>
                    <div className="history-card-footer">Started {formatDateTime(workflow.started_at)}</div>
                  </button>
                ))}
              </div>
            </section>
          </aside>
        </div>
      </div>
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

function pickPreferredWorkflow(
  workflows: WorkflowRunSummary[],
  selectedWorkflowId: number | null,
) {
  if (workflows.length === 0) {
    return null;
  }

  const runningWorkflow = workflows.find((workflow) => workflow.status === 'running');
  if (runningWorkflow) {
    return runningWorkflow;
  }

  if (selectedWorkflowId != null) {
    const existing = workflows.find((workflow) => workflow.id === selectedWorkflowId);
    if (existing) {
      return existing;
    }
  }

  return workflows[0];
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
    </label>
  );
}

function MetricCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric-card">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function OverviewRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="overview-row">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
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
