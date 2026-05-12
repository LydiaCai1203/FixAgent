import type { ProjectSummary, PullRequestSummary } from './types';

export function formatDateTime(value: string) {
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

export function deriveProjectNameFromProject(project: ProjectSummary, prs: PullRequestSummary[]) {
  const relatedPr = prs.find((pr) => pr.project_id === project.id);
  return deriveProjectNameFromPrUrl(relatedPr?.pr_url) ?? project.project_name;
}

export function deriveProjectNameFromPrUrl(prUrl?: string | null) {
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

export function buildCommitUrl(prUrl: string, commitSha: string): string | null {
  try {
    const url = new URL(prUrl);
    const parts = url.pathname.split('/').filter(Boolean);
    if (parts.length >= 2) {
      return `${url.origin}/${parts[0]}/${parts[1]}/commit/${commitSha}`;
    }
  } catch {
    return null;
  }
  return null;
}
