# Playwright Color Configuration for CI

## Problem

When running Playwright tests locally, the output includes bright colors that help distinguish the semantic meaning of the text (pass/fail/skip/etc.). However, in GitHub Actions, this color formatting may not appear as expected, making it harder to quickly identify test results.

## Root Cause

GitHub Actions doesn't recognize the terminal as a TTY (teletypewriter) device, so Playwright and other tools don't output colors by default.

## Solution

### 1. Environment Variables

Set the following environment variables in the CI workflow:

```yaml
env:
  TERM: xterm-256color      # Enables 256-color support
  FORCE_COLOR: true         # Forces color output
  CI: true                  # Enables CI-specific behavior
```

### 2. Playwright Reporter Configuration

Configure Playwright to use the `line` reporter for CI environments:

```typescript
reporter: process.env.CI ? [
  ['line'], // Use line reporter for CI - designed for CI environments with good color support
  ['html', { open: 'never' }]
] : [
  ['html', { open: 'never' }]
],
```

### 3. Why the `line` Reporter?

The `line` reporter is specifically designed for CI environments and provides:
- Better color support in non-TTY environments
- Real-time output as tests run
- Clear distinction between pass/fail/skip states
- Optimized for CI log viewing

## Testing

Use the `test-playwright-colors.sh` script to verify the configuration:

```bash
./test-playwright-colors.sh
```

## Expected Results

With this configuration, GitHub Actions should display:
- ✅ Green checkmarks for passing tests
- ❌ Red X marks for failing tests
- ⏸️ Yellow indicators for skipped tests
- Clear color-coded output for better readability

## Alternative Approaches

If the above doesn't work, consider:

1. **Using the `script` command to emulate a TTY:**
   ```yaml
   run: |
     script -q -e -c "npx playwright test"
   ```

2. **Using the `list` reporter instead of `line`:**
   ```typescript
   reporter: process.env.CI ? [
     ['list'], // Alternative reporter
     ['html', { open: 'never' }]
   ] : [
     ['html', { open: 'never' }]
   ],
   ```

## References

- [Playwright Test Reporters](https://playwright.dev/docs/test-reporters)
- [GitHub Actions Color Output Discussion](https://github.com/orgs/community/discussions/26944)
- [ANSI Color Codes in CI](https://en.wikipedia.org/wiki/ANSI_escape_code)
