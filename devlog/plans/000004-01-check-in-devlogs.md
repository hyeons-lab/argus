# Check In Devlogs

## Thinking

The project originally kept devlogs local-only. The current preference is to check them in so design decisions, branch plans, and review history travel with the repository. That means the repository conventions must change at the same time: `devlog/` should no longer be ignored, and agent instructions should treat `AGENTS.md` as the canonical file while keeping `CLAUDE.md` as a compatibility redirect.

Checking in private working notes creates a disclosure risk. Before committing, scan for secrets, tokens, emails, absolute personal paths, personal-name markers, and high-entropy strings. Redact anything that identifies a private local machine or exposes credentials. Design-doc references to authentication concepts are acceptable when they describe future product behavior rather than real secrets.

## Plan

1. Copy the existing `CLAUDE.md` conventions into `AGENTS.md`.
2. Update `AGENTS.md` so devlogs are described as checked-in repository artifacts, not local-only files.
3. Replace `CLAUDE.md` with a thin redirect to `AGENTS.md`.
4. Remove the `devlog/` ignore rule from `.gitignore`.
5. Copy existing devlogs and plans into the worktree.
6. Redact personal absolute paths and private personal wording from imported devlogs.
7. Convert imported historical `HEAD` commit markers to concrete hashes so only the current docs branch devlog uses `HEAD`.
8. Scan the imported content for secrets and personal data before committing.
9. Commit as `docs: check in devlogs` and open a draft PR.
