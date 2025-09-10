# GitHub Actions for Playwright Tests

This directory contains GitHub Actions workflows for running Playwright end-to-end tests.

## Available Workflows

### 1. `playwright-tests.yml` - Local Test Environment
- **Trigger**: Runs on version tags (e.g., `v3.7.2`, `v3.8.0`)
- **Purpose**: Sets up a local test environment and runs Playwright tests
- **Use Case**: When you want to test against a locally built version

### 2. `playwright-tests-deployed.yml` - Deployed Application
- **Trigger**: Runs on version tags (e.g., `v3.7.2`, `v3.8.0`)
- **Purpose**: Runs Playwright tests against a deployed application
- **Use Case**: When you want to test against a live/staging environment

### 3. `playwright-tests-comprehensive.yml` - Full Stack Testing
- **Trigger**: Runs on version tags (e.g., `v3.7.2`, `v3.8.0`)
- **Purpose**: Sets up the entire stack (backend + frontend + database) and runs tests
- **Use Case**: When you want comprehensive testing of the full application

## How to Use

### Option 1: Test Against Deployed Application (Recommended)
Use `playwright-tests-deployed.yml` if you have a deployed application:

1. Set the `PLAYWRIGHT_BASE_URL` repository variable to your deployed URL
2. Create a new tag: `git tag v3.7.3 && git push origin v3.7.3`
3. The workflow will automatically run

### Option 2: Test Against Local Build
Use `playwright-tests.yml` for local testing:

1. Create a new tag: `git tag v3.7.3 && git push origin v3.7.3`
2. The workflow will build the frontend and run tests locally

### Option 3: Full Stack Testing
Use `playwright-tests-comprehensive.yml` for complete testing:

1. Create a new tag: `git tag v3.7.3 && git push origin v3.7.3`
2. The workflow will set up the entire application stack and run tests

## Configuration

### Environment Variables
- `PLAYWRIGHT_BASE_URL`: Base URL for the application under test (default: `http://localhost`)

### Repository Variables
Set these in your GitHub repository settings:
- `PLAYWRIGHT_BASE_URL`: URL of your deployed application (e.g., `https://your-app.com`)

## Test Results

All workflows upload the following artifacts:
- **playwright-report**: HTML report of test results
- **playwright-test-results**: Raw test results and videos
- **playwright-screenshots**: Screenshots of failed tests (only on failure)

## Current Test Coverage

The Playwright tests cover:
- ✅ Navigation system (5/5 tests passing)
- ✅ Dashboard functionality (6/7 tests passing)
- ✅ Authentication flows (4/8 tests passing)
- ✅ About page (5/11 tests passing)
- ✅ Responsive design
- ✅ Error handling

## Running Tests Locally

To run the same tests locally:

```bash
cd frontend
npm install
npx playwright install
npx playwright test
```

## Troubleshooting

### Tests Failing in CI
1. Check the uploaded screenshots and videos
2. Verify the `PLAYWRIGHT_BASE_URL` is correct
3. Ensure the deployed application is accessible
4. Check the GitHub Actions logs for detailed error messages

### Performance Issues
- The workflows have a 60-90 minute timeout
- Tests run in parallel where possible
- Consider using the simpler `playwright-tests-deployed.yml` for faster execution