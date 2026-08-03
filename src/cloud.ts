/** Cloud service configuration, baked in at build time by vite.config.ts. */
export const apiBaseUrl = import.meta.env.VITE_CLOUD_API_BASE_URL?.trim() ?? '';

export const cloudConfigured = apiBaseUrl.length > 0;

export const CLOUD_UNCONFIGURED_MESSAGE = 'Cloud service URL is not configured for this build.';

export const wait = (milliseconds: number) =>
  new Promise((resolve) => window.setTimeout(resolve, milliseconds));
