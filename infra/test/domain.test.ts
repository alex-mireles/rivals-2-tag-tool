import { gzipSync } from 'node:zlib';
import { describe, expect, it } from 'vitest';
import { normalizeGamerTag, normalizeTournamentSlug, normalizeUserSlug, sha256, validateUpload } from '../src/domain.js';

describe('cloud domain validation', () => {
  it('normalizes exact lookup keys', () => {
    expect(normalizeGamerTag('  PlÜp ')).toBe('plüp');
    expect(normalizeUserSlug('https://www.start.gg/user/abc_123/')).toBe('user/abc_123');
    expect(normalizeTournamentSlug('https://start.gg/tournament/genesis/event/rivals')).toBe('tournament/genesis');
  });

  it('accepts a valid compressed upload', () => {
    const raw = Buffer.from('r2tag payload');
    const compressed = gzipSync(raw);
    const result = validateUpload({
      tagName: 'Player', saveVersion: 12, compression: 'gzip',
      uncompressedSha256: sha256(raw), compressedBase64: compressed.toString('base64'),
    }, 1024, 1024);
    expect(result.uncompressedSize).toBe(raw.length);
  });

  it('rejects corrupt, oversized, and mismatched uploads', () => {
    expect(() => validateUpload({ tagName: 'x', saveVersion: 1, compression: 'gzip', uncompressedSha256: '0'.repeat(64), compressedBase64: 'bm90LWd6aXA=' }, 1024, 1024)).toThrow();
    const raw = Buffer.alloc(2048, 1);
    const compressed = gzipSync(raw);
    expect(() => validateUpload({ tagName: 'x', saveVersion: 1, compression: 'gzip', uncompressedSha256: sha256(raw), compressedBase64: compressed.toString('base64') }, 1024, 1024)).toThrow();
  });
});
