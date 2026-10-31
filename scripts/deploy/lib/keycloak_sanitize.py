"""The production keys and secrets removed from a restored Keycloak database: the SQL that
scripts/deploy/release-lab.py and scripts/qa/deploy-rc-to-qa.sh run on their copy.

`python3 scripts/deploy/lib/keycloak_sanitize.py` prints it.
"""

# Run on the restored Keycloak database before Keycloak starts, in one transaction.
# Removed: the production keys and secrets that would let the lab sign, decrypt or join
# as production, or reach production's identity providers and mail server:
#   - identity providers, their mappers, configuration and account links (no sign-in
#     through Google or Facebook, and none of their client secrets);
#   - the realms' key providers (RSA, HMAC and AES private keys): Keycloak makes new
#     ones, so nothing production signed or encrypted is valid in the lab, and nothing
#     the lab signs is valid in production;
#   - every confidential client's secret, replaced by a random one (the realm import
#     then sets the clients it manages from the lab's Secret), and rotated secrets;
#   - the cluster's JGroups CA (server_config crt_jgroups, with its private key) and the
#     JGroups member addresses (jgroups_ping): Keycloak makes a new certificate;
#   - offline sessions, revoked tokens, login and admin events, SMTP settings;
#   - secret-looking values (secret, password, credential, token, key) in the remaining
#     components' and authenticators' configuration, such as a federation bind password.
# Kept on purpose, for realistic sign-in tests, so the copy still holds production
# secrets of its users: users, attributes, groups, roles and consents, and in credential
# their password hashes, TOTP seeds and WebAuthn keys; fed_user_credential;
# user_federation_config; and a SAML client's private key in client_attributes, if any.
# Tables a Keycloak version lacks are skipped.
KEYCLOAK_SANITIZE_SQL = """
DO $$
DECLARE t text;
BEGIN
  IF to_regclass('public.component') IS NOT NULL THEN
    IF to_regclass('public.component_config') IS NOT NULL THEN
      DELETE FROM public.component_config WHERE component_id IN
        (SELECT id FROM public.component WHERE provider_type = 'org.keycloak.keys.KeyProvider');
      UPDATE public.component_config SET value = ''
        WHERE name ~* '(secret|password|credential|token|key)';
    END IF;
    DELETE FROM public.component WHERE provider_type = 'org.keycloak.keys.KeyProvider';
  END IF;
  IF to_regclass('public.client') IS NOT NULL THEN
    UPDATE public.client SET secret = replace(gen_random_uuid()::text, '-', '')
                                      || replace(gen_random_uuid()::text, '-', '')
      WHERE NOT public_client AND secret IS NOT NULL;
  END IF;
  IF to_regclass('public.client_attributes') IS NOT NULL THEN
    DELETE FROM public.client_attributes WHERE name LIKE 'client.secret.rotated%';
  END IF;
  IF to_regclass('public.server_config') IS NOT NULL THEN
    DELETE FROM public.server_config WHERE server_config_key = 'crt_jgroups';
  END IF;
  IF to_regclass('public.authenticator_config_entry') IS NOT NULL THEN
    UPDATE public.authenticator_config_entry SET value = ''
      WHERE name ~* '(secret|password|credential|token|key)';
  END IF;
  FOREACH t IN ARRAY ARRAY['idp_mapper_config', 'identity_provider_mapper', 'identity_provider_config',
                           'federated_identity', 'identity_provider', 'offline_client_session',
                           'offline_user_session', 'realm_smtp_config', 'revoked_token',
                           'event_entity', 'admin_event_entity', 'jgroups_ping'] LOOP
    IF to_regclass('public.' || t) IS NOT NULL THEN
      EXECUTE format('DELETE FROM public.%I', t);
    END IF;
  END LOOP;
END $$;
"""


if __name__ == '__main__':
    print(KEYCLOAK_SANITIZE_SQL)
