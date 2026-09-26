-- Restore the fallback shim from 2025-02-02-000002. Column defaults keep the built-in uuidv7().
CREATE OR REPLACE FUNCTION public.uuidv7()
RETURNS UUID AS $$
BEGIN
    RETURN gen_random_uuid();
END;
$$ LANGUAGE plpgsql;
