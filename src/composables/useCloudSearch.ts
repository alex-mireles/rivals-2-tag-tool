import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { apiBaseUrl, describeCloudError, isRateLimited, wait } from '../cloud';
import type { CloudTagMetadata, TournamentTagPage } from '../types';

/**
 * Looking up published cloud tags, by player or by tournament.
 *
 * A tournament scan walks start.gg 50 entrants at a time behind a 1 rps route
 * throttle, so a large bracket takes tens of seconds. It is cancellable, and
 * the owning view must call `cancel()` on unmount — module scope means there is
 * no component lifecycle to stop an in-flight scan on its own.
 */

const PAGE_THROTTLE_MS = 1100;

/**
 * The route throttle is stage-wide, not per client: self-pacing just under
 * 1 rps keeps a lone TO inside it, but a second person scanning anything at the
 * same time puts both over. Throttled pages are therefore an expected event on
 * a long scan, not an error — back off and ask again.
 */
const RETRY_BACKOFF_MS = [1500, 3000, 6000];

const query = ref('');
const results = ref<CloudTagMetadata[]>([]);
const selected = ref<Set<string>>(new Set());
const isWorking = ref(false);
const progress = ref('');
const tournamentName = ref('');
const tournamentSlug = ref('');

let cancelled = false;

const selectedResults = computed(() =>
  results.value.filter((tag) => selected.value.has(tag.startggUserId)),
);
const allSelected = computed(
  () => results.value.length > 0 && selected.value.size === results.value.length,
);

function setResults(items: CloudTagMetadata[], selectAll = true) {
  results.value = items;
  selected.value = selectAll ? new Set(items.map((item) => item.startggUserId)) : new Set();
}

function toggleSelected(id: string) {
  const next = new Set(selected.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  selected.value = next;
}

function toggleAll() {
  selected.value = allSelected.value
    ? new Set()
    : new Set(results.value.map((item) => item.startggUserId));
}

function cancel() {
  cancelled = true;
}

function reset() {
  cancel();
  // The query goes too: a player name is not a tournament, and leaving it
  // behind when the search type changes only invites a nonsense search.
  query.value = '';
  setResults([]);
  progress.value = '';
  tournamentName.value = '';
  tournamentSlug.value = '';
}

async function searchPlayer(): Promise<string> {
  if (!query.value.trim()) return '';
  isWorking.value = true;
  progress.value = '';
  tournamentName.value = '';
  tournamentSlug.value = '';
  // Same reason as the tournament scan: a failed lookup must not leave the
  // previous player's results sitting under the new error.
  setResults([]);
  try {
    setResults(
      await invoke<CloudTagMetadata[]>('cloud_search_tags', { apiBaseUrl, query: query.value }),
    );
    if (!results.value.length) {
      progress.value = 'No published cloud tag matched that exact username or profile.';
    }
    return '';
  } catch (error) {
    return describeCloudError(error);
  } finally {
    isWorking.value = false;
  }
}

/** One page of the scan, retried while the service says it is throttling us. */
async function fetchPage(page: number): Promise<TournamentTagPage> {
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await invoke<TournamentTagPage>('cloud_tournament_tags', {
        apiBaseUrl,
        slug: query.value,
        page,
      });
    } catch (error) {
      if (!isRateLimited(error) || attempt >= RETRY_BACKOFF_MS.length || cancelled) throw error;
      progress.value = `Page ${page} was rate limited — waiting to retry…`;
      await wait(RETRY_BACKOFF_MS[attempt]);
      if (cancelled) throw error;
    }
  }
}

async function searchTournament(): Promise<string> {
  if (!query.value.trim()) return '';
  isWorking.value = true;
  cancelled = false;
  // Clear up front rather than only on success: a scan that fails would
  // otherwise leave the *previous* tournament's players on screen under an
  // error about this one, and they'd pack under the old name and slug.
  setResults([]);
  progress.value = '';
  tournamentName.value = '';
  tournamentSlug.value = '';
  // Deduplicated because a player entered in several events appears per event.
  const matches = new Map<string, CloudTagMetadata>();
  let page = 1;
  try {
    let totalPages = 1;
    do {
      const response = await fetchPage(page);
      totalPages = response.totalPages;
      tournamentName.value = response.tournamentName;
      tournamentSlug.value = response.tournamentSlug;
      for (const tag of response.matches) matches.set(tag.startggUserId, tag);
      progress.value = `${response.tournamentName}: scanned page ${page} of ${Math.max(
        totalPages,
        1,
      )} · ${matches.size} uploaded tag(s) found`;
      page += 1;
      if (page <= totalPages && !cancelled) await wait(PAGE_THROTTLE_MS);
    } while (page <= totalPages && !cancelled);

    setResults([...matches.values()]);
    if (cancelled && results.value.length) {
      progress.value = `Scan stopped · ${results.value.length} uploaded tag(s) found so far`;
    }
    return '';
  } catch (error) {
    // Publish what the scan did reach. A 40-page bracket that dies on page 31
    // has already produced nearly all of its results, and dropping the Map on
    // the floor means starting the whole slow scan over for nothing.
    setResults([...matches.values()]);
    const detail = describeCloudError(error);
    if (!results.value.length) return detail;
    progress.value = `Scan stopped at page ${page} · ${results.value.length} uploaded tag(s) found so far`;
    return `${detail} Showing the ${results.value.length} tag(s) found before page ${page}.`;
  } finally {
    isWorking.value = false;
  }
}

export function useCloudSearch() {
  return {
    query,
    results,
    selected,
    selectedResults,
    allSelected,
    isWorking,
    progress,
    tournamentName,
    tournamentSlug,
    setResults,
    toggleSelected,
    toggleAll,
    searchPlayer,
    searchTournament,
    cancel,
    reset,
  };
}
