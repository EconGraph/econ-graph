/**
 * Bridge between the React auth context and non-React request code such as `executeGraphQL`.
 * The `AuthProvider` registers a provider; request code asks it for the current access token.
 */

export type AccessTokenProvider = () => Promise<string | null>;

let provider: AccessTokenProvider | null = null;

/**
 * Registers the function that supplies access tokens.
 * @param next - The provider.
 * @returns A function that unregisters it, unless another provider has replaced it since.
 */
export function setAccessTokenProvider(next: AccessTokenProvider): () => void {
  provider = next;
  return () => {
    if (provider === next) {
      provider = null;
    }
  };
}

/**
 * Returns a current access token, renewing an expired one first.
 * @returns The token, or null when the user is signed out.
 */
export async function getAccessToken(): Promise<string | null> {
  return provider ? provider() : null;
}
