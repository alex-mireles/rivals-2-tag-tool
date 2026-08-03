import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { apiBaseUrl, wait } from '../cloud';
import type { CloudTagMetadata, CloudUser } from '../types';

/**
 * The start.gg sign-in session.
 *
 * Module-scoped so navigating away from the share screen no longer silently
 * drops the session and forces a browser round trip on every visit. The token
 * lives only here — never on disk, since this app is expected to run on shared
 * tournament PCs.
 */

interface AuthRequest {
  requestId: string;
  pollToken: string;
  authorizationUrl: string;
  expiresAt: number;
}

interface PollResult {
  status: 'pending' | 'complete';
  sessionToken?: string;
  user?: CloudUser;
}

const sessionToken = ref('');
const signedInUser = ref<CloudUser | null>(null);
const publishedTag = ref<CloudTagMetadata | null>(null);

async function signIn(onProgress: (message: string) => void): Promise<void> {
  const request = await invoke<AuthRequest>('cloud_begin_auth', { apiBaseUrl });
  await openUrl(request.authorizationUrl);
  onProgress('Complete authentication in your browser…');

  while (Date.now() / 1000 < request.expiresAt) {
    await wait(1500);
    const poll = await invoke<PollResult>('cloud_poll_auth', {
      apiBaseUrl,
      requestId: request.requestId,
      pollToken: request.pollToken,
    });
    if (poll.status === 'complete' && poll.sessionToken && poll.user) {
      sessionToken.value = poll.sessionToken;
      signedInUser.value = poll.user;
      const owned = await invoke<CloudTagMetadata[]>('cloud_search_tags', {
        apiBaseUrl,
        query: poll.user.slug,
      });
      publishedTag.value =
        owned.find((tag) => tag.startggUserId === poll.user?.startggUserId) ?? null;
      onProgress('');
      return;
    }
  }
  throw new Error('Authentication request expired.');
}

async function signOut() {
  if (sessionToken.value) {
    await invoke('cloud_end_session', { apiBaseUrl, sessionToken: sessionToken.value }).catch(
      () => undefined,
    );
  }
  sessionToken.value = '';
  signedInUser.value = null;
  publishedTag.value = null;
}

export function useCloudAuth() {
  return { sessionToken, signedInUser, publishedTag, signIn, signOut };
}
