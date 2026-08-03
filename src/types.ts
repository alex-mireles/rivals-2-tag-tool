export interface SaveFileState {
  savePath: string;
  savePathError: boolean;
  tagNames: string[];
  hasLoaded: boolean;
}

export interface TagPreview {
  path: string;
  tag_name: string;
  version: number | null;
  compatible: boolean;
}

export interface PreviewResult {
  save_version: number | null;
  previews: TagPreview[];
}

export interface ImportResult {
  imported: string[];
  skipped: string[];
  incompatible: string[];
}

export interface CloudUser {
  startggUserId: string;
  slug: string;
  gamerTag: string;
}

export interface CloudTagMetadata {
  startggUserId: string;
  startggSlug: string;
  gamerTag: string;
  tagName: string;
  saveVersion: number | null;
  uncompressedSha256: string;
  compressedSize: number;
  uncompressedSize: number;
  updatedAt: string;
}

export interface TournamentTagPage {
  tournamentName: string;
  tournamentSlug: string;
  eventNames: string[];
  page: number;
  totalPages: number;
  totalEntrants: number;
  matches: CloudTagMetadata[];
}
