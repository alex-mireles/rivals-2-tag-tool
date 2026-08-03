import { createHash } from 'node:crypto';
import { gunzipSync } from 'node:zlib';

export interface UploadInput {
  tagName: string;
  saveVersion: number | null;
  compression: string;
  uncompressedSha256: string;
  compressedBase64: string;
}

export function normalizeGamerTag(value: string): string {
  return value.trim().normalize('NFKC').toLocaleLowerCase('en-US');
}

export function normalizeUserSlug(value: string): string | null {
  let candidate = value.trim();
  try {
    const url = new URL(candidate);
    if (!/(^|\.)start\.gg$/i.test(url.hostname)) return null;
    candidate = url.pathname;
  } catch {
    // Treat non-URLs as a slug.
  }
  candidate = candidate.replace(/^\/+|\/+$/g, '');
  const match = candidate.match(/(?:^|\/)user\/([a-zA-Z0-9_-]+)$/);
  return match ? `user/${match[1]}` : null;
}

export function normalizeTournamentSlug(value: string): string | null {
  let candidate = value.trim();
  try {
    const url = new URL(candidate);
    if (!/(^|\.)start\.gg$/i.test(url.hostname)) return null;
    candidate = url.pathname;
  } catch {
    // Treat non-URLs as a slug.
  }
  candidate = candidate.replace(/^\/+|\/+$/g, '');
  if (candidate.length > 200) return null;
  const match = candidate.match(/(?:^|\/)tournament\/([a-zA-Z0-9_-]+)/);
  if (match) return `tournament/${match[1]}`;
  return /^[a-zA-Z0-9_-]+$/.test(candidate) ? `tournament/${candidate}` : null;
}

export function sha256(value: string | Buffer): string {
  return createHash('sha256').update(value).digest('hex');
}

export function validateUpload(
  input: UploadInput,
  maxCompressedBytes: number,
  maxUncompressedBytes: number,
): { compressed: Buffer; uncompressedSize: number } {
  if (!input || typeof input !== 'object') throw new Error('Invalid upload body');
  if (typeof input.tagName !== 'string' || input.tagName.trim().length === 0 || input.tagName.length > 128) {
    throw new Error('Invalid tag name');
  }
  if (input.saveVersion !== null && (!Number.isInteger(input.saveVersion) || input.saveVersion < 0)) {
    throw new Error('Invalid save version');
  }
  if (input.compression !== 'gzip') throw new Error('Unsupported compression');
  if (!/^[a-f0-9]{64}$/i.test(input.uncompressedSha256)) throw new Error('Invalid SHA-256');
  if (typeof input.compressedBase64 !== 'string' || input.compressedBase64.length === 0) {
    throw new Error('Missing compressed payload');
  }

  const compressed = Buffer.from(input.compressedBase64, 'base64');
  const canonicalInput = input.compressedBase64.replace(/=+$/, '');
  if (compressed.toString('base64').replace(/=+$/, '') !== canonicalInput) throw new Error('Invalid base64 payload');
  if (compressed.length === 0 || compressed.length > maxCompressedBytes) throw new Error('Compressed payload too large');

  let uncompressed: Buffer;
  try {
    uncompressed = gunzipSync(compressed, { maxOutputLength: maxUncompressedBytes + 1 });
  } catch {
    throw new Error('Invalid or oversized gzip payload');
  }
  if (uncompressed.length === 0 || uncompressed.length > maxUncompressedBytes) {
    throw new Error('Uncompressed payload too large');
  }
  if (sha256(uncompressed) !== input.uncompressedSha256.toLowerCase()) throw new Error('SHA-256 mismatch');
  return { compressed, uncompressedSize: uncompressed.length };
}
