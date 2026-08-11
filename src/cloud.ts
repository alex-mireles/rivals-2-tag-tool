/** Cloud service configuration, baked in at build time by vite.config.ts. */
export const apiBaseUrl = import.meta.env.VITE_CLOUD_API_BASE_URL?.trim() ?? '';

export const cloudConfigured = apiBaseUrl.length > 0;

export const CLOUD_UNCONFIGURED_MESSAGE = 'Cloud service URL is not configured for this build.';

/**
 * Sentinel the backend returns for a throttled request (HTTP 429/503) instead
 * of a message. Must match `RATE_LIMITED` in `src-tauri/src/commands/cloud.rs`:
 * the tournament scan retries on it, so it has to be matchable rather than read.
 */
export const RATE_LIMITED_ERROR = 'cloud-rate-limited';

export const isRateLimited = (error: unknown) => String(error) === RATE_LIMITED_ERROR;

/** Turn a backend error into something worth showing a user. */
export const describeCloudError = (error: unknown) =>
  isRateLimited(error)
    ? 'The cloud service is busy right now — wait a moment and try again.'
    : String(error);

export const wait = (milliseconds: number) =>
  new Promise((resolve) => window.setTimeout(resolve, milliseconds));
