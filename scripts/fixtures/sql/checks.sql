-- Per-table checks for scripts/fixtures/build-sanitized-fixture.sh, run on
-- each sanitized database after the steps in steps.sql (and on the CI subset
-- after it is cut). fixture_sanitize.checks() returns one row per check that
-- fails, with the number of offending rows; values are never returned. The
-- scan for e-mail addresses and secret-shaped values in every text and JSON
-- column is fixture_sanitize.scan() in functions.sql.

CREATE FUNCTION fixture_sanitize.checks()
RETURNS TABLE (check_name text, n bigint)
LANGUAGE plpgsql AS $$
DECLARE
    c record;
    pseudo_email constant text := '^user-[0-9a-f]{16}@example\.invalid$';
    pseudo_ip constant text := '^10\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$';
BEGIN
    CREATE TEMP TABLE IF NOT EXISTS fixture_checks (kind text, check_name text, sql text) ON COMMIT DROP;
    TRUNCATE fixture_checks;
    INSERT INTO fixture_checks VALUES
        -- app (public.users)
        ('app', 'users.email is not a pseudonym',
         format('SELECT count(*) FROM users WHERE email !~ %L', pseudo_email)),
        ('app', 'users.name is not a pseudonym',
         $q$SELECT count(*) FROM users WHERE name !~ '^User [0-9a-f]{16}$'$q$),
        ('app', 'users.password_hash or avatar_url is set',
         'SELECT count(*) FROM users WHERE password_hash IS NOT NULL OR avatar_url IS NOT NULL'),
        ('app', 'users.organization is not a pseudonym',
         $q$SELECT count(*) FROM users WHERE organization !~ '^Organization [0-9a-f]{16}$'$q$),
        ('app', 'users.provider_id is neither a UUID nor a pseudonym',
         $q$SELECT count(*) FROM users
            WHERE provider_id !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
              AND provider_id !~ '^p-[0-9a-f]{16}$'$q$),
        ('app', 'user_sessions is not empty', 'SELECT count(*) FROM user_sessions'),
        ('app', 'audit_logs.user_name is not a pseudonym',
         $q$SELECT count(*) FROM audit_logs WHERE user_name !~ '^User [0-9a-f]{16}$'$q$),
        ('app', 'audit_logs.ip_address is not a stand-in, or user_agent is set',
         format('SELECT count(*) FROM audit_logs WHERE ip_address !~ %L OR user_agent IS NOT NULL', pseudo_ip)),
        ('app', 'security_events.ip_address is not a stand-in, or user_agent is set',
         format('SELECT count(*) FROM security_events WHERE ip_address !~ %L OR user_agent IS NOT NULL', pseudo_ip)),
        ('app', 'security_events.user_email is not a pseudonym',
         format('SELECT count(*) FROM security_events WHERE user_email !~ %L', pseudo_email)),
        -- keycloak (public.user_entity)
        ('keycloak', 'user_entity.username is not a pseudonym',
         $q$SELECT count(*) FROM user_entity
            WHERE service_account_client_link IS NULL
              AND username !~ '^user-[0-9a-f]{16}(@example\.invalid)?$'$q$),
        ('keycloak', 'user_entity first or last name is not a pseudonym',
         $q$SELECT count(*) FROM user_entity
            WHERE first_name <> 'User' OR last_name !~ '^[0-9a-f]{16}$'$q$),
        ('keycloak', 'user_entity.email is not a pseudonym',
         format('SELECT count(*) FROM user_entity WHERE email !~ %L', pseudo_email)),
        ('keycloak', 'user_attribute holds an attribute other than locale',
         $q$SELECT (SELECT count(*) FROM user_attribute WHERE name <> 'locale')
                 + fixture_sanitize.count_rows(ARRAY['fed_user_attribute'])$q$),
        ('keycloak', 'credentials remain',
         $q$SELECT fixture_sanitize.count_rows(ARRAY['credential', 'fed_user_credential',
            'user_ver_credential', 'fed_user_ver_credential', 'issued_ver_credential',
            'fed_issued_ver_credential'])$q$),
        ('keycloak', 'federated identities remain',
         $q$SELECT fixture_sanitize.count_rows(ARRAY['federated_identity', 'broker_link'])$q$),
        ('keycloak', 'sessions or tokens remain',
         $q$SELECT fixture_sanitize.count_rows(ARRAY['offline_client_session', 'offline_user_session',
            'auth_session', 'root_auth_session', 'revoked_token', 'single_use_object',
            'client_initial_access', 'client_session_auth_status', 'client_session_note',
            'client_session_prot_mapper', 'client_session_role', 'client_user_session_note',
            'client_session', 'user_session_note', 'user_session'])$q$),
        ('keycloak', 'events or login failures remain',
         $q$SELECT fixture_sanitize.count_rows(ARRAY['event_entity', 'admin_event_entity',
            'login_failure', 'outbox_entry'])$q$),
        ('keycloak', 'a client, identity provider, SMTP or LDAP secret is not a placeholder',
         $q$SELECT (SELECT count(*) FROM client
                    WHERE (secret <> '' AND secret <> 'fixture-client-secret')
                       OR registration_token IS NOT NULL)
                 + (SELECT count(*) FROM client_attributes WHERE name LIKE 'client.secret.rotated%')
                 + (SELECT count(*) FROM identity_provider_config
                    WHERE name = 'clientSecret' AND value <> 'fixture-idp-client-secret')
                 + (SELECT count(*) FROM realm_smtp_config
                    WHERE name = 'password' AND value <> 'fixture-smtp-password')
                 + (SELECT count(*) FROM component_config
                    WHERE name = 'bindCredential' AND value <> 'fixture-bind-credential')$q$),
        ('keycloak', 'realm key providers remain',
         $q$SELECT count(*) FROM component WHERE provider_type = 'org.keycloak.keys.KeyProvider'$q$),
        ('keycloak', 'server state (JGroups certificate, cluster members) remains',
         $q$SELECT fixture_sanitize.count_rows(ARRAY['server_config', 'jgroups_ping', 'cluster_event'])$q$),
        -- data (public.economic_series)
        ('data', 'crawl_attempts headers that look like credentials remain',
         $q$SELECT count(*) FROM crawl_attempts
            WHERE fixture_sanitize.has_secret_header(request_headers)
               OR fixture_sanitize.has_secret_header(response_headers)$q$);

    FOR c IN SELECT * FROM fixture_checks LOOP
        IF (c.kind = 'app' AND fixture_sanitize.has_table('public.users'))
           OR (c.kind = 'keycloak' AND fixture_sanitize.has_table('public.user_entity'))
           OR (c.kind = 'data' AND fixture_sanitize.has_table('public.economic_series')) THEN
            EXECUTE c.sql INTO n;
            IF n > 0 THEN
                check_name := c.check_name;
                RETURN NEXT;
            END IF;
        END IF;
    END LOOP;
END
$$;
