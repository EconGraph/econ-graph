This document is a comprehensive list of AI developer expectations, based on mistakes I have noticed, that all AI agents should have in their context while working on this project.  It is inspired by Cursor's memory feature, but at some point it is good for it to be static, so it can be shared across projects.


* When starting a new task, you should always create a new branch off of main for work related to that task.  You should do it by running:
```
git fetch
git branch -b your_agent_name/descriptive_branch_name_for_your_task origin/main
```
This avoids multiple agents checking out main from different git worktrees at the same time.
* **ALWAYS use `git mv` when moving files**: When you need to move or rename files, use `git mv old_path new_path` instead of `mv old_path new_path`. This ensures git properly tracks the file move as a rename operation rather than a delete + add, which preserves file history and makes the git log cleaner. Using `mv` followed by `git add` can cause git to lose track of file history.
* Please make sure to not use interactive merge, or other interactive git commands.  You just hang and your human developer has to rescue you by typing `:wq`.  If you're working overnight, that's hours of work lost.
* Always write lots of tests, and make sure that every feature you write is covered.  This means you can iterate against them and fix them until it works.
* Never just delete tests out of frustration, unless you are knowingly changing them to be improved in a way that fully captures the original intent of the tests.
* If you are struggling with a more comprehensive test, write smaller more targeted tests to test your hypotheses before making changes, and please do small proof of concepts rather than trying to iterate on the whole program.  Write a utility that you suspect captures the essence of the bug, test that, and see if your proposed feature fixes it.
* Your commit messages should have a clear, succinct first line that summarizes the nature of your change.  Then, follow it by a detailed description of what you did and why.  Ideally, prefix it with `chore:`, `feature:`, `fix:`, etc.  Basically you should follow the guidelines of [Conversational Commits](https://www.conventionalcommits.org/en/v1.0.0/).
* Similar guidelines for titles and descriptions of github PRs.  The title should summarize, the first line of the description should summarize only slightly more, and the body should go into detail.  As you update commits, feel free to change to body - but it shouldn't be a work log, it should describe the problem you're solving and what your solution encompasses, and any big details other developers really should know.
* You've been given access to the `gh` commandline tool.  I shouldn't have to constantly remind you of that.
* Your human overseer prefers the socratic method when talking about potential solutions.  If I question you, I want to hear your reasoning, or for you to convince me what you're doing is not a bug with citations to documentations of the library you're using - I don't want you to just change it.
* Do not delete existing features of the codebase just because you're frustrated at integrating them with yours.  You have to make them work together.
* If you are tagging or giving version numbers, they can only move forwards, not backwards.  And you should realistically use [Semantic Versioning](https://semver.org/) where possible.  But all version numbers begin with a 0 major number until your human overseer has decided to call something 1.0.0, since before 1.0.0 rapid development is occuring and there is no sense in constantly incrementing major version numbers into the 100s when you are rapidly experimenting.
* You should go on long loops of making more tests, iterating on them, fixing the code, if stuck making smaller code changes and more focused tests to test core hypotheses, and continuing to iterate until your task is accomplished.  If you are truly stuck in a loop and you are unable to discover new ideas, then stop as you are flailing.  But preferably, you really try harder rather than stopping for user input.  Ideally you should be able to work independently and productively for hours at a time.
* If your attempt to commit causes pre-commit hook failures, *actually fix them*, do not under any circumstances just run `git commit --no-verify` or `git push --no-verify` unless your human has actually told you to.
* If you notice your chosen port isn't working (e.g. for a dockerized postgres) please choose a random port, don't just keep incrementing it by one.  And by no means try to kill whatever is already on the port you want.  Just pick a different port, and don't hardcode it, allow it to easy be changed by environment variable so you can easily change port numbers on the fly.
* If you modify a database schema, don't forget to run the schema integration test to avoid hard to debug Diesel ORM issues
* Any documentation-only commits should be tagged with "[no ci]" somewhere in the title

## Modular Codebase Organization

* **Modular Codebase Organization**: Follow the established modular directory structure. Place scripts and tools in their appropriate domain directories (e.g., `ci/scripts/` for CI/CD tools, `scripts/` for general project scripts, `backend/` for Rust code, `frontend/` for React code). This improves maintainability and makes the codebase more navigable.
* **Domain-Specific Tooling**: When creating validation scripts, testing tools, or automation scripts, place them in the appropriate domain directory rather than a generic `scripts/` folder. This makes the codebase more modular and easier to understand.
* **Consistent Directory Structure**: Maintain consistency with the existing modular structure. If you're unsure where to place a new script or tool, follow the pattern of similar existing tools in the codebase.

## Database Testing Best Practices

* When working with database tests, always ensure that `clean_database()` methods return `Result<(), Error>` and handle errors properly. Never use `.expect()` in test utilities as this can mask real issues.
* Database constraint violations in tests often indicate poor test isolation. Always ensure tests clean up after themselves and use unique identifiers to avoid conflicts.
* When you see "duplicate key value violates unique constraint" errors in CI, this usually means either:
  * The database cleaning isn't working properly
  * Tests are running in parallel and interfering with each other
  * Test data setup is using non-unique identifiers
* Always handle `Result` types properly in test utilities. Use `map_err()` to convert errors and `?` to propagate them, not `.expect()`.

## Rust Compiler Warning Management

* Fix unused variable warnings systematically by prefixing with underscore (`_variable_name`) when the variable is intentionally unused.
* Handle "unused `Result` that must be used" warnings by either:
  * Properly handling the Result with `match` or `if let`
  * Using `let _ = result;` when you intentionally want to ignore the result
  * Using `result.expect("meaningful error message")` only when you're certain it won't fail
* Run `cargo fix --lib -p package_name --tests` to automatically apply many warning fixes.
* Never ignore compiler warnings in CI - they often indicate real issues that should be addressed.

## Test Categorization Guidelines

**CRITICAL: Proper test categorization is essential for CI performance and reliability.**

### Unit Tests (Fast Tests - No External Dependencies)
* **Location**: Inside crate `tests.rs` modules or `#[cfg(test)]` modules
* **Characteristics**:
  * No database connections
  * No external API calls
  * No network requests
  * No file system operations (except temporary test files)
  * No environment variables for external services
  * Pure function testing
  * Configuration validation
  * Data structure validation
  * Business logic testing
* **Examples**:
  * Testing data source configuration structs
  * Testing data parsing functions
  * Testing validation logic
  * Testing mathematical calculations
  * Testing string manipulation

### Integration Tests (Slow Tests - External Dependencies)
* **Location**: `tests/` directory at project root (for Rust) or dedicated integration test directories
* **Characteristics**:
  * Database connections and operations
  * External API calls
  * Network requests
  * File system operations
  * Environment variable dependencies
  * Full system integration testing
  * End-to-end workflows
* **Examples**:
  * Testing database CRUD operations
  * Testing API client functionality
  * Testing full service workflows
  * Testing external service integrations
  * Testing file upload/download
  * Testing authentication flows

### Test Naming and Organization
* **Unit tests**: Use simple `#[test]` attribute
* **Integration tests**: Use `#[tokio::test]` for async operations and `#[serial]` for database tests
* **File naming**: Integration tests should be clearly named (e.g., `*_integration_tests.rs`)
* **Test descriptions**: Clearly indicate if a test requires external dependencies in comments

### CI Impact
* **Smoke tests** (fast unit tests) run on every commit and must pass quickly
* **Integration tests** run less frequently and can take longer
* **Never mix** database-dependent tests in unit test suites
* **Always move** tests that require external dependencies to integration test directories

### Common Mistakes to Avoid
* ❌ Putting database tests in unit test modules
* ❌ Making API calls in unit tests
* ❌ Using `TestContainer` in unit tests
* ❌ Testing external service integrations in unit tests
* ❌ Using environment variables for external services in unit tests
* ✅ Move any test requiring external dependencies to integration tests
* ✅ Keep unit tests fast and isolated
* ✅ Use proper test categorization from the start

## Port Configuration and Environment Variables

**CRITICAL: Centralize all port numbers and service URLs as environment variables for maintainability and consistency.**

### Centralized Configuration
* **Single Source of Truth**: All port numbers, URLs, and service endpoints must be defined in centralized configuration files
* **Environment Variables**: Use environment variables for all service configurations instead of hardcoding values
* **Configuration Files**: Maintain configuration in dedicated files like `ci-env.config` or `.env` files
* **Inheritance Pattern**: All services, databases, and test frameworks should inherit configuration from the centralized source

### Port Management Best Practices
* **Avoid Port Conflicts**: Use different ports for different test suites to prevent conflicts
* **Dynamic Port Assignment**: When possible, use dynamic port assignment or environment variable overrides
* **Port Naming Convention**: Use descriptive names like `BACKEND_PORT`, `FRONTEND_PORT`, `DATABASE_PORT`
* **Test-Specific Ports**: Use separate ports for different test suites (e.g., `BACKEND_SMOKE_DB_PORT=5433`, `BACKEND_INTEGRATION_DB_PORT=5447`)

### Configuration Structure
* **Core Services**: Define base ports for main services (backend, frontend, database)
* **Derived URLs**: Build service URLs from base configuration (e.g., `BACKEND_URL=http://localhost:${BACKEND_PORT}`)
* **Health Check Endpoints**: Define health check URLs consistently (e.g., `BACKEND_HEALTH_URL=${BACKEND_URL}/health`)
* **API Endpoints**: Centralize API endpoint definitions (e.g., `GRAPHQL_ENDPOINT=${BACKEND_URL}/graphql`)

### Implementation Requirements
* **CI/CD Integration**: All CI workflows must use environment variables for port configuration
* **Docker Configuration**: Docker containers must use environment variables for port mapping
* **Test Configuration**: All test frameworks (Playwright, Jest, etc.) must inherit port configuration
* **Service Discovery**: Use environment variables for service-to-service communication URLs

### Common Mistakes to Avoid
* ❌ Hardcoding port numbers in multiple places
* ❌ Using different ports for the same service across different environments
* ❌ Not updating all references when changing port numbers
* ❌ Creating port conflicts between different test suites
* ❌ Using inconsistent URL patterns across services
* ✅ Define all ports in one centralized configuration file
* ✅ Use environment variables throughout the codebase
* ✅ Use descriptive, consistent naming conventions
* ✅ Test port configuration changes across all environments

### Configuration File Example
```bash
# Core Service Ports
BACKEND_PORT=8080
FRONTEND_PORT=3000
DATABASE_PORT=5432

# Derived URLs
BACKEND_URL=http://localhost:${BACKEND_PORT}
FRONTEND_URL=http://localhost:${FRONTEND_PORT}
DATABASE_URL=postgresql://postgres:password@localhost:${DATABASE_PORT}/econ_graph_test

# Health Check Endpoints
BACKEND_HEALTH_URL=${BACKEND_URL}/health
FRONTEND_HEALTH_URL=${FRONTEND_URL}

# Test-Specific Ports (to avoid conflicts)
BACKEND_SMOKE_DB_PORT=5433
BACKEND_INTEGRATION_DB_PORT=5447
```

## CI Failure Debugging

* When CI fails, always check the detailed logs using `gh run view RUN_ID --log-failed` to see the actual error messages.
* Common CI failure patterns:
  * Database constraint violations: Usually test isolation issues
  * Container timeouts: Often Docker resource issues or network problems
  * Compilation errors: Usually missing dependencies or syntax issues
  * Test failures: Check if tests are properly isolated and cleaned up
  * Smoke test failures: Often caused by unit tests trying to access external dependencies
* Always fix the root cause, not just the symptoms. For example, if you see database constraint violations, fix the test isolation rather than just changing the test data.
* If smoke tests fail due to external dependencies, move the problematic tests to integration tests.

## Pull Request Management and Single Concern Principle

* **Create Small, Focused PRs**: When working on larger projects or fixing multiple issues, always break down your work into small, focused PRs that address a single concern. This makes code review easier, reduces risk, and allows for independent testing and merging.
* **Independent Fixes Require Separate Branches**: If you identify multiple independent issues while working on a task, create separate branches for each fix:
  * Each branch should be created from `main` (not from your working branch)
  * Each branch should contain only the changes related to one specific issue
  * This allows each fix to be reviewed, tested, and merged independently
* **Logical Separation Examples**:
  * **Bug fixes** should be separate from **feature additions**
  * **Code quality improvements** (warnings, formatting) should be separate from **functional changes**
  * **Test reorganization** should be separate from **test fixes**
  * **Configuration changes** should be separate from **code changes**
* **PR Creation Process**:
  * Create each PR with a clear, descriptive title that summarizes the single concern
  * Write detailed descriptions explaining the problem, solution, and impact
  * Include specific file changes and reasoning
  * Suggest merge order if PRs have dependencies
* **Benefits of Single Concern PRs**:
  * **Easier code review** - Reviewers can focus on one specific change
  * **Reduced risk** - Smaller changes are less likely to introduce new bugs
  * **Better testing** - Each change can be tested independently
  * **Cleaner git history** - Each commit/PR has a clear, single purpose
  * **Faster CI** - Smaller changes run faster and are less likely to fail
* **When to Combine Changes**: Only combine changes in a single PR when they are:
  * Tightly coupled and cannot be separated
  * Part of the same logical feature
  * Required to work together to achieve a single goal
* **Example of Good Separation**:
  * ❌ **Bad**: "Fix CI failures and improve test organization" (two concerns)
  * ✅ **Good**: "Fix database cleanup table name bug" + "Move MCP tests to integration tests" (two separate PRs)

## Debugging Methodology and Avoiding False Confidence

**CRITICAL: Never claim to have "fixed all issues" without comprehensive verification.**

### Systematic Debugging Approach
* **One Issue at a Time**: Focus on fixing ONE specific failure completely before moving to the next
* **Verify Each Fix**: After each fix, wait for CI results and confirm the specific issue is resolved
* **Don't Assume**: Never assume that fixing one issue automatically fixes related issues
* **Test Locally First**: When possible, reproduce and fix issues locally before pushing to CI
* **Read Error Messages Carefully**: Don't just scan error messages - read them completely and understand the root cause

### False Confidence Anti-Patterns
* ❌ **"I've fixed all the issues"** - This is almost never true in complex systems
* ❌ **"This should resolve everything"** - Complex systems have interdependencies you may not see
* ❌ **"The tests should pass now"** - Always verify with actual test results
* ❌ **Making multiple changes simultaneously** - Makes it impossible to know which change fixed what
* ❌ **Assuming correlation equals causation** - Just because you made a change doesn't mean it fixed the problem

### Proper Debugging Workflow
1. **Identify the specific failure** - Get the exact error message and understand what's failing
2. **Reproduce locally** - Try to reproduce the issue in your local environment
3. **Make ONE targeted fix** - Address only the specific root cause you identified
4. **Test the fix** - Verify the fix works locally if possible
5. **Push and wait for CI** - Let CI run and confirm the specific issue is resolved
6. **Only then move to the next issue** - Don't start fixing other things until you've confirmed this fix worked

### Communication Standards
* **Be Honest About Uncertainty**: Say "I believe this fixes X" not "This fixes everything"
* **Acknowledge What You Don't Know**: "I'm not sure if this will fix Y, but it should fix X"
* **Provide Evidence**: Reference specific error messages and explain how your fix addresses them
* **Set Expectations**: "This should fix the Docker build issue, but there may be other failures"

### When You're Wrong (And You Will Be)
* **Acknowledge Mistakes Immediately**: Don't double down on incorrect assumptions
* **Learn from Each Failure**: Each failed fix teaches you something about the system
* **Ask for Help Sooner**: If you're stuck in a loop, ask for guidance rather than continuing to guess
* **Document What Didn't Work**: Keep track of approaches that failed so you don't repeat them

### Red Flags That You're Going Down the Wrong Path
* You're making multiple changes without testing each one
* You're claiming to fix "everything" without specific evidence
* You're not waiting for CI results before making more changes
* You're getting frustrated and making larger, riskier changes
* You're ignoring specific error messages in favor of general assumptions

### Success Metrics
* ✅ **Specific Issues Resolved**: You can point to exact error messages that are now gone
* ✅ **Incremental Progress**: Each change moves you closer to a working system
* ✅ **Verifiable Fixes**: You can explain exactly how each fix addresses a specific problem
* ✅ **Honest Communication**: You accurately represent what you know and what you don't know
