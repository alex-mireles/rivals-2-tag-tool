export interface SaveFileState {
  savePath: string;
  savePathError: boolean;
  tagNames: string[];
  hasLoaded: boolean;
}

/// A start.gg account linked to exported tags.
export interface StartggLinkValue {
  slug: string;
  tag: string;
}
