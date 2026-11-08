# Contributing to memcastle

## Setup

The toolchain is managed by [mise](https://mise.jdx.dev/);
every tool is pinned to an exact build in `mise.lock`, so a local run and a CI run use the same versions.

```bash
mise install          # the tools and Git hooks
```

Mise's postinstall hook installs both `pre-commit` and `commit-msg` hooks.
Prek installs Commitizen and its Emotional plugin in an isolated Python environment
for commit-message validation.

Windows CI places build outputs and temporary files on a ReFS Dev Drive.
The shared setup action exports `TMP`, `TEMP` and `TMPDIR` for all subsequent
steps and child processes, with temporary storage outside the cached target directory.

### Mr Boxington build caching

[Mr Boxington](https://mr-boxington.jdx.dev/setup) is enabled through mise's
native Rust option (requires mise 2026.9.2+). Rustup provides the stable channel
declared in `rust-toolchain.toml`; keep that channel and the mise Rust version
in sync when changing a pin, preserving existing components and other options.
Explicit `RUSTUP_TOOLCHAIN` selections win. In GitHub Actions, the runtime
preserves the rustup default selected by `dtolnay/rust-toolchain`, including
MSRV jobs. Mise still ensures its stable install and declared components exist;
the install request stays stable so locked installs remain valid.

```bash
mise exec -- cargo build --locked
mise exec -- cargo nextest run
mise exec -- mbx doctor
```

**Mise tasks have mbx built in:** `mise build`, `mise test`, and their explicit
forms `mise run build` and `mise run test` already route Cargo through mbx; no
extra `mbx` prefix is needed. Plain terminal `cargo` requires effective
mise activation or shims; a direct rustup Cargo proxy bypasses it. Coding agents
should use `mise exec` rather than assume the terminal's PATH. Native integration
does not require `mbx setup` or configure rust-analyzer. Doctor can warn about a
missing standalone shim even when wrapping works; verify real build summaries.
If personal environment paths shadow the wrapper in task shells, enable
`[settings] activate_aggressive = true` in ignored `mise.local.toml` or use the
explicit `mise exec -- cargo ...` route.

An existing compiler wrapper makes mbx delegate rather than cache. Inspect
Cargo configuration and `RUSTC_WRAPPER` before changing one; an authorized
project-only `[build] rustc-wrapper = ""` override can clear an inherited wrapper.
Environment-selected wrappers still take precedence and prevent mbx caching.

CI uses `jdx/mr-boxington-action` instead of sccache and rust-cache. Its default
GitHub `target` payload includes Cargo outputs and registry/git downloads, so
an additional rust-cache action is unnecessary. Successful main pushes populate
the cache; all pull requests only restore to avoid expensive archive creation
and PR-scoped entries competing for the cache budget. PR-only cross-build jobs
can restore compatible baselines but do not populate caches of their own.
Native builds also use compiler caching.
Publication jobs use `backend: local` without shared compiler-output restores
or saves. No separate remote cache service or local-to-CI connection is configured.

Local compiler actions, eligible links, build scripts and C/C++ compilations
are cached by default. Compilation scheduling, in-flight deduplication and
learned private incremental reuse are also enabled. For concurrent agents or
test suites, optionally add `[scheduler] tests = true` in workspace `.mbx.toml`;
it schedules tests, not their outcomes. Review nextest timeouts if tests queue.

Automatic cache collection follows your global mbx policy or mbx's bounded
defaults. Configure machine-specific budgets in `~/.config/mbx/config.toml`,
not in the project. For example, to cap the combined cache at 60 GiB:

```toml
[gc]
max_total_size = "60GiB"
max_size = "45GiB"
incremental_max_size = "15GiB"
```

The incremental allowance includes generated sources. Low disk space can trigger
collection below these limits; active state is protected. `MBX_GC_AUTO=0` in your
environment disables automatic collection, including low-disk collection.
Preview cleanup with `mise exec -- mbx gc --dry-run`.

Managed target placement stays disabled through `MBX_TARGET_VIEWS=0`: ordinary
worktree `target/` directories are outside these cache budgets. Private incremental
state lives in the mbx cache and survives worktree deletion and `cargo clean`.
Use `mise exec -- mbx clean /absolute/retired/worktree` to remove a retired
checkout's private state while keeping shared compiler outputs.

To demonstrate reuse, run equivalent builds in two fresh temporary target
directories using `--target-dir` and `MBX_SUMMARY=full`. Require positive mbx hits
on the second build; Cargo freshness is not a hit. Diagnose with
`mise exec -- mbx explain --last` and remove only your temporary targets.

To undo, remove or disable `mr_boxington` first and run `mise reshim`, then remove
the mbx tool/lock entry and introduced environment settings. Restore rustup-only
ownership by removing the added mise Rust entry, preserving unrelated pins and
options. Remove any wrapper/scheduler and local activation overrides you added;
keep the shared cache. Set the template answer `mbx = false` and review the
rendered update to restore the original CI cache actions too.

Dev and test profiles use `debug = "line-tables-only"` to retain line-number
backtraces with smaller outputs and less linker IO. Full debugger variable
information requires rebuilding with `CARGO_PROFILE_DEV_DEBUG=2` and/or
`CARGO_PROFILE_TEST_DEBUG=2`. Changing profiles causes a one-time rebuild;
old artifacts do not shrink in place.

Check/Clippy produces metadata, whereas nextest needs linked test executables.
Some compilation between them is expected. Features, compiler versions, coverage
instrumentation and release profiles also change build inputs. Keep these stable
within an edit loop and select Cargo packages/targets for focused checks; a
nextest runtime filter alone does not necessarily reduce compiled targets.

Before committing or opening a PR, choose checks relevant to your changes;
see [Before you push](#before-you-push) for guidance. Clippy runs through
`mise lint` and in CI, rather than in the commit hooks. `mise ci` is an
optional comprehensive check.

## Tasks

| Command | What it does |
|---|---|
| `mise run default` | Format, lint, build and test |
| `mise run build` | Build the binary |
| `mise cli <args>` | Run `memcastle` from source (passes flags through) |
| `mise run setup` | Install `memcastle` into `~/.cargo/bin` |
| `mise run test` | Run the basic tests, without the WebAssembly suite (accepts nextest selectors) |
| `mise run test:wasm` | Run the WebAssembly suite, every `tests/wasm_*.rs` binary (slow, needs the `wasm32-wasip2` target) |
| `mise run sources:check` | Build and conformance-test every source under `sources/` |
| `mise run sources:test -- <name>` | Build and conformance-test one source under `sources/` |
| `mise run sources:package` | Exercise local source-bundle packaging into `target/bundled-sources`; Core releases carry no sources |
| `mise run integrations:check` | Package the shared skills and check the checkout and release asset layouts |
| `mise run integrations:build` | Prepare any Core-shipped declarative integrations (currently none) |
| `mise run integrations:package` | Lay out the shared skills that ship with releases in `target/bundled-integrations` |
| `mise run web:check` | Typecheck, test, build and package the web UI, then serve it from a real daemon (needs bun and node) |
| `mise run web:build` | Build `web/` into `web/dist`, for `--assets-dir "$PWD"` with `web.enable` (needs bun and node) |
| `mise run web:dev` | Run the dashboard's dev server against the running daemon (needs bun and node) |
| `mise run web:package` | Lay out the dashboard that ships with releases in `target/bundled-web` (needs bun and node) |
| `mise run cover` | Run the tests with coverage |
| `mise run format` | Format |
| `mise run format:check` | Check the formatting without rewriting |
| `mise run lint` | Clippy, warnings denied |
| `mise run lint:actions` | actionlint over the workflows |
| `mise run lint:md` | rumdl over AGENTS.md, CONTRIBUTING.md, README.md, docs/ and the skills' `SKILL.md` files |
| `mise run spell` | typos |
| `mise run guards` | The `architecture` and `architecture-rule-tests` prek hooks: ast-grep scan and native rule tests for invariants 1, 2, 4, 5, 8 and 11 |
| `mise run snapshots` | Review pending insta snapshots |
| `mise run check` | Every lint, the guards, both test suites, the sources and the integrations, without modifying the working tree |
| `mise run ci` | `check` plus the documentation build |
| `mise run docs` | Serve the documentation locally |
| `mise run docs:build` | Build the documentation |
| `mise run changelog` | Preview the changelog for unreleased commits |
| `mise run release` | Show the version the next release would take |
| `mise run ship:validate` | Check the release setup against what gh-ship requires |
| `mise run tpl:check` | Has the template moved? Exits 1 if merging it would change anything |
| `mise run tpl:update` | Bring the rendered template ref up to date |
| `mise run tpl:diff` | Show what merging the template would change |

`mise <task>` is a shorthand for `mise run <task>`, but a builtin subcommand of the same name wins it silently —
which is why the format task is `format` and not `fmt` (`mise fmt` formats `mise.toml`),
and why running the binary is `mise cli` and not `mise run` (`mise run` runs a task).
Prefer the explicit `mise run <task>` in scripts: mise can claim a new name in any release.

## Documentation

Documentation is part of the change, not a follow-up:
a pull request that changes a flag, a setting, an MCP tool or a user-visible behaviour updates the page that describes it.
User pages live in `docs/`, are listed in the `nav` of `zensical.toml`, and use one sentence per line.
Architecture diagrams are Mermaid.
[Development](docs/development.md#documentation) has the conventions, and `mise run docs` previews the site.

## Before you push

Choose checks based on the files changed and the risk. Run relevant tasks
individually rather than launching every check in parallel by default.

- **Rust code:** run `mise lint` (Clippy) and the relevant tests through
  `mise test`, which accepts nextest selectors. Target affected packages
  and tests instead of running the whole suite by default. For example,
  replacing the placeholders with the affected package and test name:

  ```sh
  mise test -p <package> -E 'test(<test_name>)'
  ```

- **Documentation, workflows or tooling:** run the relevant checks, such as
  `mise lint:md`, `mise spell` or `mise lint:actions`, and
  `mise docs:build` when documentation changes warrant a build.
  Documentation-only changes do not require Rust lint or tests.

Broaden validation when the change warrants it. `mise ci` is available as
an optional comprehensive check, not a prerequisite for committing, pushing or
opening a PR.

If a local check is unavailable or prohibitively expensive, proceed with the
commit or PR and report which checks ran, which were skipped and why in the
handoff or PR description. CI provides full validation.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/), enforced by
Commitizen with [Emotional](https://github.com/noirbizarre/emotional) on `commit-msg`.
The hook checks Emotional's Conventional Commit grammar; subject case and length
remain authoring conventions. The type selects the changelog section, and a `!`
or a `BREAKING CHANGE:` footer drives the version bump — so the message is part
of the release, not paperwork around it.

The official typos hooks check source files on `pre-commit` and commit messages
on `commit-msg`. Message spelling is checked without rewriting the message, so
fix any reported errors before retrying the commit.

## Releases

Releases are run by [gh-ship](https://github.com/noirbizarre/gh-ship).
**Never bump a version or push a tag by hand.**

1. A push to `main` triggers 🚢 Ship, where `gh ship run` selects preparation.
2. `prepare` dispatches 🚀 Prepare Release,
   which asks git-cliff for the next version, writes `CHANGELOG.md`, bumps `Cargo.toml`, commits,
   and uploads a `ship.release.json` artifact describing what would ship.
3. gh-ship opens (or updates) the Release PR from `release/next`.
   Review it.
4. Merging it triggers 🚢 Ship again, where `gh ship run` selects publication:
   it tags the merge commit, creates a draft release,
   dispatches 📦 Publish Release to attach the binaries, then makes the release public.

Nothing to release is the normal case for step 1, and costs one workflow run reporting `changed: false`.

`gh ship validate` runs on every pull request, so a broken release contract fails on the PR rather than mid-release.

### What a release contains

📦 Publish Release attaches, for a tag `<tag>`:

- the raw executables, `memcastle_<tag>_<platform>[.exe]`, which the AUR recipe, the Homebrew formula and
  `cargo binstall` fetch by name, so renaming them breaks those;
- a tarball for each Unix platform, `memcastle_<tag>_<platform>.tar.gz`, in the native-package layout
  (`bin/`, `share/doc/memcastle/`, `share/memcastle/{sources,integrations,skills,web}/`);
- the shared skills on their own, `memcastle_<tag>_skills.tar.gz`, and the web UI,
  `memcastle_<tag>_web.tar.gz`, which one asset each serves to the AUR package and the Homebrew formula;
- no Core source or agent-integration archive: provider plugins publish those modules independently
  (see [Publishing and installing sources](docs/publishing-sources.md#the-official-registry));
- `.deb` and `.rpm` packages for linux-amd64 and linux-arm64, `memcastle_<tag>_<platform>.{deb,rpm}`, built by nfpm
  from `packaging/nfpm/nfpm.yaml` (binary, systemd user unit, shell completions, documentation,
  shared skills and the web UI)
  and smoke-tested before upload;
  CI builds them from stub binaries on every pull request, through the same `packaging/nfpm/build.sh`,
  so a broken packaging config fails on the pull request and not mid-release;
- `memcastle-<tag>.cdx.json`, a CycloneDX bill of materials;
- `SHA256SUMS` over all of the above, with build-provenance attestations for every file and an SBOM attestation for the
  executables and tarballs, which `gh attestation verify` checks.

The release is built to be reproducible, and two things keep it that way.
The Rust toolchain, `cross` and the runner images are pinned to exact versions,
so moving any of them is an edit to the workflow and not something that happens between two releases.
The publish job also installs Bun at the version recorded in `mise.lock`, alongside Node 24,
so the web UI packages with the same tool version used in local checks.
Build paths are remapped to fixed names, and the tarballs have a fixed order, timestamps and ownership.
CI does not rebuild the release to check this, because two LTO release builds are too slow for a pull request.
Bump the toolchain in `RUSTUP_TOOLCHAIN` and the `dtolnay/rust-toolchain` refs together.
See [ADR-013](docs/adr/013-release-packaging-and-asset-resolution.md) for the reasoning.

### Trying a release build without publishing

Where 📦 Publish Release is dispatched decides whether it publishes.
gh-ship dispatches it on the release tag, which builds, attests and uploads as above.
A run dispatched on a branch is a trial: it builds every leg, writes a table of build time and binary size to the
run's summary, and never reaches the publish job.

```sh
gh workflow run publish-release.yaml --ref <branch> -f ref=trial
```

On a branch, `ref` is only a label for the asset names.
There are no trial options: to try a different profile, edit `[profile.release]` in `Cargo.toml` on the branch
(or change the workflow itself), and dispatch that branch, which runs the branch's own copy of the workflow.
A trial's binaries are ordinary artifacts, so compare their sizes against the release assets as well as the times.

The release profile is thin LTO with one codegen unit, chosen from trials of 0.3.0's build:
fat LTO took 53 to 59 minutes on the macOS legs, thin LTO under 28 on every leg for a binary 8% larger.
Run a trial before changing `lto` or `codegen-units` in `[profile.release]`, and compare both time and size.

The pinned release workflow is template-owned (see below), so a change that belongs to every project generated
from the template should be made in rust.tpl.

### Recovering a failed draft

If 📦 Publish Release fails after its five platform builds, leave the tag and draft release intact.
Fix the publishing step on `main` through a reviewed pull request, then dispatch 🛠️ Recover Release on `main` with
the existing tag and failed publish run ID:

```sh
gh workflow run recover-release.yaml --ref main -f tag=<tag> -f run_id=<failed-publish-run-id>
```

The recovery verifies that the saved binaries came from the tag, packages the tagged source with the fixed publish
workflow, runs the package smoke tests and attestations, and uploads the assets to the existing draft.
It never publishes the draft automatically: check that every expected asset, `SHA256SUMS`, and the attestations are
present and valid, then make the existing release public with `gh release edit <tag> --draft=false`.
If a platform build failed or its artifact expired, repair the build and use the normal release process instead.

### Repository requirements

The release jobs authenticate as a GitHub App, not with `GITHUB_TOKEN` —
the default token cannot trigger workflows, so a Release PR it authored would show no CI results.
That means the repository needs:

- a `release` environment holding the variable `APP_CLIENT_ID` and the secret
  `APP_PRIVATE_KEY`.
- the corresponding GitHub App installed on `memcastle/memcastle` with
  Contents, Actions, Pull requests and Issues read/write permissions.

Preparation runs `gh ship sign` to re-create the release commit with GitHub's signature.

Also configure:

- squash-merge settings of `squash_merge_commit_title: PR_TITLE` and
  `squash_merge_commit_message: BLANK`, so the squash commit subject is the
  Conventional Commit title from `.github/ship.yml`.
- a `homebrew` environment holding the secret `TAP_TOKEN`, scoped to push to
  `noirbizarre/homebrew-tap` only.
- an `aur` environment holding the secret `AUR_SSH_PRIVATE_KEY`, for the AUR
  account that owns the `memcastle-bin` package.

### Documentation and coverage services

Documentation is published to `memcastle/memcastle.github.io` on its `gh-pages` branch.
Configure that repository's Pages source to deploy from the branch root.
The publishing App needs Contents write access to that repository.

Onboard `memcastle/memcastle` to Codecov for coverage and test results.

## This repository is generated from a template

The toolchain, hooks, CI and release workflows come from [rust.tpl](https://github.com/noirbizarre/rust.tpl):

```bash
git tpl status         # is there a template update pending?
mise run tpl:update    # advance refs/tpl/<id> — HEAD and index untouched; may save changed answers
mise run tpl:diff      # read what merging it would change
git tpl merge          # take it
```

`tpl:update` advances the rendered ref and may save changed answers in `.config/git.tpl.toml`.
Rendered files reach your branch only through the merge.

`mise install` provides git-tpl through the declared `cargo:` tool. With
cargo-binstall available, mise downloads its release archive rather than
compiling the libgit2 dependency from source.

Files carrying template-owned content — `mise.toml`, `prek.toml`, `Cargo.toml` —
end with a `# --- project-specific ...` marker (`project-specific tasks` in `mise.toml`, `project-specific hooks` in `prek.toml`).
Add below it;
Git's 3-way merge then preserves your additions across updates.
A fix that belongs to every project belongs in the template, not here.
