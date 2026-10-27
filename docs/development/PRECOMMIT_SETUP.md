# Pre-commit Hooks Setup

The repository's hooks run formatting, lint, security, and file checks before commits and pushes. Install them in each checkout to enable automatic execution.

## Install in a checkout

Run the commands below from the repository root. You need Python 3 with `venv` support, Git, Bash, the Rust toolchain in [rust-toolchain.toml](../../rust-toolchain.toml), and the Node.js version in [.nvmrc](../../.nvmrc) with npm. The Rust toolchain includes rustfmt and Clippy; the security hook also needs `cargo-audit`.

One way to install `pre-commit` without changing system Python packages is to use a virtual environment outside the repository:

```bash
python3 -m venv "$HOME/.venvs/econgraph-precommit"
source "$HOME/.venvs/econgraph-precommit/bin/activate"
python -m pip install pre-commit
pre-commit --version
```

Prepare the local hook dependencies, then install the Git hooks:

```bash
cargo install cargo-audit --locked
(cd frontend && npm ci)
(cd admin-frontend && npm ci)
pre-commit install
```

The [configuration](../../.pre-commit-config.yaml) sets `default_install_hook_types` to `pre-commit` and `pre-push`, so this installs both hooks. They run on `git commit` and `git push`, respectively. The first run of a remote hook downloads and prepares its environment, so it needs network access. Security audits also need access to their advisory services.

## Run checks manually

With the virtual environment activated, run the hooks on staged files before committing:

```bash
pre-commit run
```

To select all tracked files instead, use either command:

```bash
pre-commit run --all-files
./scripts/pre-commit.sh run --all-files
```

The [helper script](../../scripts/pre-commit.sh) forwards its arguments to the `pre-commit` executable on `PATH`; it does not install or locate a separate copy.

`pre-commit run` accepts one hook ID at a time. These examples select all tracked files for each hook:

```bash
# Rust
pre-commit run rust-fmt --all-files
pre-commit run rust-clippy --all-files

# Frontend
pre-commit run frontend-prettier --all-files
pre-commit run frontend-eslint --all-files

# Security
pre-commit run rust-audit --all-files
pre-commit run npm-audit --all-files
```

Manual runs use the `pre-commit` stage by default. To select the push stage explicitly:

```bash
pre-commit run --hook-stage pre-push --all-files
```

## What the hooks do

Hooks are selected by filename patterns and exclusions in [.pre-commit-config.yaml](../../.pre-commit-config.yaml). A hook can be skipped when no selected file matches. Selecting staged files or using `--all-files` determines which hooks are triggered; local commands can still operate on a whole workspace or project.

- **Rust:** `rust-fmt` runs `cargo fmt --all` and rewrites backend files. `rust-clippy` checks all backend targets and features with the configured lint exceptions. Both are triggered by matching backend Rust files.
- **Frontend:** Prettier checks formatting; ESLint checks JavaScript and TypeScript; `frontend-jsdoc` checks JSDoc with a warning budget; `typescript-check` runs `tsc --noEmit`. These hooks target `frontend`; admin frontend formatting, lint, and type-check hooks are commented out. The ESLint and JSDoc commands scan their configured source globs, and TypeScript checks the project.
- **Security:** `rust-audit` checks backend dependencies when the Cargo manifest or lockfile matches. `npm-audit` runs the shared audit helper for both frontends, using each package's allowlist, when package files, the helper, or allowlists match.
- **General files:** Hooks check YAML, JSON, TOML, merge conflicts, and added files over 5,000 KB. Trailing-whitespace, end-of-file, and mixed-line-ending hooks can rewrite files; line endings are fixed to LF. Markdown and text files are excluded from the whitespace and end-of-file fixers, and other exclusions are recorded in the configuration.
- **Documentation:** `markdownlint` checks Markdown using [.markdownlint.json](../../.markdownlint.json), also used by the Docs Checks workflow.
- **Migrations:** `migration-date-order-check` runs the backend migration-order validator when migration SQL files match.

`fail_fast: false` lets the remaining hooks run after one fails. Review all failures and inspect any changes before restaging files and retrying the commit or push.

## Troubleshooting

### Command not found or Python import errors

Activate the virtual environment used during installation, then check the executable:

```bash
source "$HOME/.venvs/econgraph-precommit/bin/activate"
command -v pre-commit
pre-commit --version
```

If you used another installer, put that installation's executable directory on `PATH`. If the executable exists but cannot import `pre_commit`, reinstall the package in the Python environment that owns the executable.

### Formatting failures

These commands start from the repository root and use subshells so the first directory change does not affect the next command:

```bash
(cd backend && cargo fmt --all)
(cd frontend && npx prettier --write "src/**/*.{ts,tsx,js,jsx,json,css,md}")
```

Review the diff, stage the intended fixes, and rerun the failing hook. Rust formatting can change files beyond those originally staged.

### Other hook failures

Read the failing hook's output and check its command in the configuration. Repair missing tools, dependencies, or configuration before retrying. If a hook itself is broken, report the error to the maintainer with the hook ID and reproduction command. Follow repository instructions for handling the blocker; do not bypass hooks to get a commit or push through.

## Update hook versions

Maintainers can update remote hook revisions with:

```bash
pre-commit autoupdate
pre-commit run --all-files
```

Review and commit the configuration changes and any resulting fixes. This updates the configured remote hooks; local Rust and npm tools are managed separately. Run `pre-commit install` in a checkout when the configured Git hook types change.
