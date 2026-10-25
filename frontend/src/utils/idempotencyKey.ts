/**
 * Idempotency keys for creates the backend deduplicates (ECO-439): one per thing being
 * created, reused only when retrying that same create.
 */

/**
 * A new random key, a version-4 UUID.
 *
 * `crypto.randomUUID` exists only in secure contexts (HTTPS or localhost), so a build served
 * over plain HTTP on a LAN address falls back to `crypto.getRandomValues`, which exists in
 * every context.
 * @returns The key.
 */
export function newIdempotencyKey(): string {
  if (typeof globalThis.crypto.randomUUID === 'function') return globalThis.crypto.randomUUID();
  const bytes = globalThis.crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
  bytes[8] = (bytes[8] & 0x3f) | 0x80; // RFC 4122 variant
  const hex = Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
