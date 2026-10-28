-- The sanitize steps of scripts/fixtures/build-sanitized-fixture.sh, in the
-- order they run. Each starts at a line
--     --@step <name> <kind>
-- and runs on every restored database of that kind: app (has public.users),
-- keycloak (has public.user_entity), data (has public.economic_series), or
-- all. A database can be of more than one kind (APP_DATABASE=shared puts the
-- app and data tables in econ_graph). All steps for one database run in one
-- transaction, with triggers and foreign-key triggers off
-- (session_replication_role = replica), so updated_at columns keep their
-- values. Helpers are in functions.sql; what each step leaves behind is
-- checked by checks.sql and the scan in functions.sql.

--@step app_audit_actors app
-- audit_logs: the actor's name becomes the pseudonym of their account (the
-- one users.name gets, keyed by the account's e-mail address), or, for a row
-- whose account is gone, of the stored name. The IP address becomes a
-- stand-in in 10.0.0.0/8 (the same for the same address); the user agent is
-- removed. user_id is a random UUID and stays, so rows still join to users.
UPDATE audit_logs a
SET user_name = fixture_sanitize.pseudo_name(
        coalesce((SELECT u.email FROM users u WHERE u.id = a.user_id), 'name:' || a.user_name)),
    ip_address = fixture_sanitize.pseudo_ip(a.ip_address),
    user_agent = NULL;

--@step app_security_events app
-- security_events: the IP address becomes a stand-in in 10.0.0.0/8 and the
-- user agent is removed. user_email is rewritten by all_emails, like every
-- other address.
UPDATE security_events
SET ip_address = fixture_sanitize.pseudo_ip(ip_address),
    user_agent = NULL;

--@step app_users app
-- users: the name becomes 'User <h>', h keyed by the account's e-mail address
-- (Keycloak's first and last name become 'User' and the same h, so the name
-- in a token matches). The avatar URL and password hash are removed; an
-- organization becomes 'Organization <h>'; a provider id that isn't a UUID
-- (an OAuth subject such as Google's) becomes 'p-<h>'. The e-mail address
-- itself is rewritten by all_emails.
UPDATE users
SET name = fixture_sanitize.pseudo_name(email),
    avatar_url = NULL,
    password_hash = NULL,
    organization = CASE WHEN organization IS NULL THEN NULL
                        ELSE 'Organization ' || fixture_sanitize.h16(organization) END,
    provider_id = CASE
        WHEN provider_id IS NULL THEN NULL
        WHEN provider_id ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' THEN provider_id
        ELSE 'p-' || fixture_sanitize.h16(provider_id) END;

--@step app_sessions app
-- Every session and its token hash.
DELETE FROM user_sessions;

--@step kc_users keycloak
-- user_entity: a username that is an e-mail address becomes that address's
-- pseudonym, any other becomes 'user-<h>'; first and last name become 'User'
-- and h of the account's e-mail address (or username), as users.name does in
-- the app database. Service accounts (service-account-<client>) keep their
-- names. E-mail addresses are rewritten by all_emails. Pending organization
-- invitations get the same treatment.
UPDATE user_entity
SET first_name = CASE WHEN first_name IS NULL THEN NULL ELSE 'User' END,
    last_name = CASE WHEN last_name IS NULL THEN NULL
                     ELSE fixture_sanitize.h16(coalesce(email, username)) END,
    username = CASE
        WHEN service_account_client_link IS NOT NULL THEN username
        WHEN username ~ '@' THEN fixture_sanitize.pseudo_email(username)
        ELSE 'user-' || fixture_sanitize.h16(username) END;
DO $$
BEGIN
    IF fixture_sanitize.has_table('public.org_invitation') THEN
        UPDATE org_invitation
        SET first_name = CASE WHEN first_name IS NULL THEN NULL ELSE 'User' END,
            last_name = CASE WHEN last_name IS NULL THEN NULL ELSE fixture_sanitize.h16(email) END;
    END IF;
END $$;

--@step kc_user_attributes keycloak
-- Free-form user attributes (phone numbers, pictures, anything an identity
-- provider mapper stored), except the locale.
DELETE FROM user_attribute WHERE name NOT IN ('locale');
SELECT fixture_sanitize.empty_tables(ARRAY['fed_user_attribute']);

--@step kc_credentials keycloak
-- Passwords, OTP and WebAuthn credentials, and verifiable credentials.
SELECT fixture_sanitize.empty_tables(ARRAY[
    'credential', 'fed_user_credential', 'user_ver_credential', 'fed_user_ver_credential',
    'issued_ver_credential', 'fed_issued_ver_credential']);

--@step kc_federated_identities keycloak
-- Links to identity provider accounts (Google subject ids and usernames, and
-- any stored provider tokens).
SELECT fixture_sanitize.empty_tables(ARRAY['federated_identity', 'broker_link']);

--@step kc_sessions keycloak
-- User and client sessions (Keycloak persists online sessions with the
-- offline ones), offline tokens, authentication sessions, revoked-token
-- records, single-use objects (action tokens) and client registration access
-- tokens. The older per-note session tables are emptied where they exist.
SELECT fixture_sanitize.empty_tables(ARRAY[
    'offline_client_session', 'offline_user_session', 'auth_session', 'root_auth_session',
    'revoked_token', 'single_use_object', 'client_initial_access',
    'client_session_auth_status', 'client_session_note', 'client_session_prot_mapper',
    'client_session_role', 'client_user_session_note', 'client_session',
    'user_session_note', 'user_session']);

--@step kc_events keycloak
-- Login and admin events (IP addresses, usernames, changed representations),
-- brute-force records (the last failing IP) and the event outbox.
SELECT fixture_sanitize.empty_tables(ARRAY[
    'event_entity', 'admin_event_entity', 'login_failure', 'outbox_entry']);

--@step kc_client_secrets keycloak
-- Client secrets become 'fixture-client-secret' and rotated secrets are
-- removed; registration tokens are removed; identity providers' client
-- secrets, the SMTP password and LDAP bind credentials get placeholders.
UPDATE client SET secret = 'fixture-client-secret' WHERE secret IS NOT NULL AND secret <> '';
UPDATE client SET registration_token = NULL WHERE registration_token IS NOT NULL;
DELETE FROM client_attributes WHERE name LIKE 'client.secret.rotated%';
UPDATE identity_provider_config SET value = 'fixture-idp-client-secret' WHERE name = 'clientSecret';
UPDATE realm_smtp_config SET value = 'fixture-smtp-password' WHERE name = 'password';
UPDATE component_config SET value = 'fixture-bind-credential' WHERE name = 'bindCredential';

--@step kc_realm_keys keycloak
-- Every realm's key providers (RSA signing and encryption keys, HMAC and AES
-- secrets). On its first start against the fixture, Keycloak generates new
-- keys for a realm that has none.
DELETE FROM component_config
WHERE component_id IN (SELECT id FROM component WHERE provider_type = 'org.keycloak.keys.KeyProvider');
DELETE FROM component WHERE provider_type = 'org.keycloak.keys.KeyProvider';

--@step kc_server_state keycloak
-- Server-wide state: the cluster's JGroups certificate and key
-- (server_config), cluster membership (jgroups_ping) and pending cluster
-- events. Keycloak creates them again on start.
SELECT fixture_sanitize.empty_tables(ARRAY['server_config', 'jgroups_ping', 'cluster_event']);

--@step data_crawl_headers data
-- crawl_attempts: request and response headers whose names suggest
-- credentials (Authorization, API keys, tokens, cookies, signatures) are
-- removed.
UPDATE crawl_attempts
SET request_headers = fixture_sanitize.strip_headers(request_headers),
    response_headers = fixture_sanitize.strip_headers(response_headers)
WHERE request_headers IS NOT NULL OR response_headers IS NOT NULL;

--@step all_url_secrets all
-- In every text and JSON column: the values of credential query parameters
-- in URLs (api_key=, token=, key=, UserID=, ...) become REDACTED.
SELECT fixture_sanitize.rewrite_all('scrub_url_secrets', fixture_sanitize.url_secret_re(), 'i');

--@step all_jwts all
-- In every text and JSON column: JSON Web Tokens become redacted.jwt.fixture.
SELECT fixture_sanitize.rewrite_all('scrub_jwts', fixture_sanitize.jwt_re());

--@step all_emails all
-- In every text and JSON column: each e-mail address outside example.invalid
-- becomes user-<h>@example.invalid, h keyed by the address, so the same
-- address gets the same pseudonym in every table and database.
SELECT fixture_sanitize.rewrite_all('scrub_emails', fixture_sanitize.email_re());
