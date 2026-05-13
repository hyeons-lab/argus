# WezTerm Term Packaging

## Thinking

The terminal-engine acceptance spike already showed that upstream `wezterm-term` is the best semantic fit for Argus daemon-owned terminal state, but the dependency path still needs to be made explicit before it graduates from spike evidence into normal implementation work.

Cargo cannot combine `git` and `path` in one dependency specification. For an upstream Git monorepo, the direct dependency path is to name the package, pin the Git revision, and let Cargo resolve the package from the upstream workspace.

The branch should prove the smallest viable path first: direct Git-pinned upstream source with an explicit crate path. A fork or artifact workflow should remain a fallback only if this path is too heavy or brittle for default development and CI.

## Plan

1. Update the feature-gated `wezterm-term` dependency to use the upstream Git URL with `.git` suffix and the reviewed commit pin.
2. Validate the feature-gated engine spike tests against the direct path dependency.
3. Record the packaging decision and any observed Cargo lockfile impact.
4. Commit and open a PR if validation passes.
