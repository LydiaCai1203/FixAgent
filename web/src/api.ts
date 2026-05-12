export const API_BASE_URL = (import.meta.env.VITE_API_BASE_URL || `${window.location.origin}/api`).replace(/\/$/, '');

export async function readApiError(response: Response) {
  try {
    const payload = (await response.json()) as { error?: string };
    return payload.error ?? `Request failed with status ${response.status}`;
  } catch {
    return `Request failed with status ${response.status}`;
  }
}

export function toErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }
  return 'Unknown error';
}
