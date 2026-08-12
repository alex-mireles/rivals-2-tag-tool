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
 *
 * Sign-in happens in the user's default browser, which we hand a URL to and
 * then lose sight of: nothing tells us the page was closed, refused, or never
 * loaded. So the in-flight state lives here too — `isSigningIn` outlives the
 * view, and the wait is always cancellable rather than pretending we can
 * detect an abandoned tab.
 */

const POLL_INTERVAL_MS = 1500;

/**
 * Every way a sign-in can end without a session, from this side of the browser
 * hand-off, is indistinguishable from any other: closed tab, denied consent,
 * or a user who wandered off. Name the likeliest cause and offer the retry.
 */
const ABANDONED_MESSAGE =
  'Sign-in was never completed — the start.gg page may have been closed. Try again.';

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
const isSigningIn = ref(false);
const signInStatus = ref('');

let cancelled = false;

/** Give up on the outstanding browser round trip. */
function cancelSignIn() {
  cancelled = true;
}

/**
 * What this user already has in the cloud, or `null` if the lookup failed.
 *
 * Runs *after* the session is live, so it must never throw: a sign-in that
 * actually succeeded reporting failure would leave the UI showing "Signed in
 * as …" beside an error. `null` costs the user only the two-step replace
 * confirmation on their next publish, which the retry below restores.
 */
async function lookUpPublishedTag(user: CloudUser): Promise<CloudTagMetadata | null> {
  try {
    const owned = await invoke<CloudTagMetadata[]>('cloud_search_tags', {
      apiBaseUrl,
      query: user.slug,
    });
    return owned.find((tag) => tag.startggUserId === user.startggUserId) ?? null;
  } catch {
    return null;
  }
}

/** Re-run the published-tag lookup for the signed-in user. */
async function refreshPublishedTag() {
  if (!signedInUser.value) return;
  publishedTag.value = await lookUpPublishedTag(signedInUser.value);
}

/**
 * Returns `'cancelled'` when the user backed out; throws for anything that
 * actually went wrong. The polling loop deliberately keeps running across
 * navigation — someone who approves in the browser after wandering back to the
 * home screen should still land signed in.
 */
async function signIn(): Promise<'signed-in' | 'cancelled'> {
  cancelled = false;
  isSigningIn.value = true;
  signInStatus.value = 'Awaiting start.gg sign-in…';
  try {
    const request = await invoke<AuthRequest>('cloud_begin_auth', { apiBaseUrl });
    await openUrl(request.authorizationUrl);

    while (Date.now() / 1000 < request.expiresAt) {
      await wait(POLL_INTERVAL_MS);
      if (cancelled) return 'cancelled';

      let poll: PollResult;
      try {
        poll = await invoke<PollResult>('cloud_poll_auth', {
          apiBaseUrl,
          requestId: request.requestId,
          pollToken: request.pollToken,
        });
      } catch (error) {
        // A 404 means the request record is gone: it aged out server-side, or
        // was already consumed. Either way this attempt can never complete, so
        // stop waiting instead of polling a dead id until our own deadline.
        if (String(error).includes('Cloud API returned 404')) break;
        throw error;
      }

      if (cancelled) return 'cancelled';
      if (poll.status === 'complete' && poll.sessionToken && poll.user) {
        sessionToken.value = poll.sessionToken;
        signedInUser.value = poll.user;
        publishedTag.value = await lookUpPublishedTag(poll.user);
        return 'signed-in';
      }
    }
    throw new Error(ABANDONED_MESSAGE);
  } finally {
    isSigningIn.value = false;
    signInStatus.value = '';
  }
}

async function signOut() {
  cancelSignIn();
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
  return {
    sessionToken,
    signedInUser,
    publishedTag,
    isSigningIn,
    signInStatus,
    signIn,
    refreshPublishedTag,
    cancelSignIn,
    signOut,
  };
}
