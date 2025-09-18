This document is a comprehensive list of AI developer expectations, based on mistakes I have noticed, that all AI agents should have in their context while working on this project.  It is inspired by Cursor's memory feature, but at some point it is good for it to be static, so it can be shared across projects.


* When starting a new task, you should always create a new branch off of main for work related to that task.  You should do it by running:
```
git fetch
git branch -b your_agent_name/descriptive_branch_name_for_your_task origin/main
```
This avoids multiple agents checking out main from different git worktrees at the same time.
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

## CI Failure Debugging

* When CI fails, always check the detailed logs using `gh run view RUN_ID --log-failed` to see the actual error messages.
* Common CI failure patterns:
  * Database constraint violations: Usually test isolation issues
  * Container timeouts: Often Docker resource issues or network problems
  * Compilation errors: Usually missing dependencies or syntax issues
  * Test failures: Check if tests are properly isolated and cleaned up
* Always fix the root cause, not just the symptoms. For example, if you see database constraint violations, fix the test isolation rather than just changing the test data.

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
