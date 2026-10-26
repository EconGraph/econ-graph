# MSW (Mock Service Worker) Setup

This directory contains the MSW setup for intercepting GraphQL requests during testing.

## Debug Output

MSW debug output is disabled by default to keep test output clean. To enable debug output when needed:

Set the Vite variable before starting Vitest (the debug flag is read when mock
modules are imported):

```bash
VITE_MSW_DEBUG=true npm test
```

## Debug Output Includes

- GraphQL request interception logs
- Operation names and variables
- Current scenario (success/error/loading)
- Response data
- Error handling

## Files

- `server.ts` - Main MSW server setup with full GraphQL support
- `simpleServer.ts` - Simplified server for environments with import issues
- `README.md` - This documentation file

## Usage

The MSW server is automatically started in `setupTests.ts` and provides mock responses for all GraphQL operations used in the admin frontend.
