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

// Whether the backend refused this session's token because the account is suspended (403
// ACCOUNT_SUSPENDED). Request code then reads public data anonymously; the auth context tells the
// user why and clears the flag when the signed-in user changes.
let accountSuspended = false;
const suspendedListeners = new Set<() => void>();

/**
 * Whether the signed-in account was found to be suspended.
 * @returns True after `setAccountSuspended(true)`, until it is cleared.
 */
export function isAccountSuspended(): boolean {
  return accountSuspended;
}

/**
 * Records whether the signed-in account is suspended, notifying subscribers of a change.
 * @param suspended - The new state.
 */
export function setAccountSuspended(suspended: boolean): void {
  if (accountSuspended === suspended) {
    return;
  }
  accountSuspended = suspended;
  suspendedListeners.forEach(listener => listener());
}

/**
 * Subscribes to changes of `isAccountSuspended()`, in the shape `useSyncExternalStore` takes.
 * @param listener - Called after each change.
 * @returns A function that unsubscribes.
 */
export function subscribeAccountSuspended(listener: () => void): () => void {
  suspendedListeners.add(listener);
  return () => {
    suspendedListeners.delete(listener);
  };
}
