/** Mirrors `SaveStatus` in src-tauri/src/commands/save_file.rs. */
export type SaveStatus =
  | 'resolving'
  | 'ready'
  | 'missing'
  | 'unreadable'
  | 'unsupported';

/** Mirrors `SaveFileInfo` in src-tauri/src/commands/save_file.rs. */
export interface SaveFileInfo {
  path: string;
  source: 'saved' | 'default' | 'none';
  status: Exclude<SaveStatus, 'resolving'>;
  defaultDir: string;
  tagNames: string[];
  saveVersion: number | null;
  error: string | null;
}

export interface TagPreview {
  path: string;
  tag_name: string;
  version: number | null;
  compatible: boolean;
  /** Set when the file itself could not be read; the row is unimportable. */
  error: string | null;
}

export interface PreviewResult {
  save_version: number | null;
  previews: TagPreview[];
}

export interface ImportResult {
  imported: string[];
  skipped: string[];
  incompatible: string[];
  removed: string[];
  backup_path: string | null;
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

/** One downloaded tag, paired with the user it belongs to. */
export interface CloudDownload {
  startggUserId: string;
  path: string;
}

export interface PackSummary {
  outputPath: string;
  entryCount: number;
  bytes: number;
  names: string[];
}

export interface UnpackResult {
  paths: string[];
  label: string | null;
  source: string | null;
  createdAt: string | null;
  declaredSaveVersion: number | null;
  manifestOk: boolean;
  entryCount: number;
  skipped: string[];
}
