## Subagent Model Routing

When dispatching subagents, always set `model` explicitly, never rely on inherit:

- `haiku` for mechanical impl (1-2 files, complete spec)
- `sonnet` for integration, multi-file, judgment calls
- `opus` for architecture, design, final review

## Decision Points

Every fork (A vs B, include or skip, scope larger or smaller, judgment-call settlement) goes through `AskUserQuestion` with 2-4 labeled options. Lead with the recommended option marked "(Recommended)" when one exists. No open-ended "what do you want" prompts, no choices buried in prose. Applies to brainstorming, design review, planning, and code-review-feedback triage.

## Finishing Branches

Default to push and create an MR. Skip the 4-option menu after `superpowers:finishing-a-development-branch` lands. Use `glab mr create`, not `gh pr create`. Still run test verification and environment detection first. If the branch is in an unusual state (failing tests, detached HEAD), surface that and present options before defaulting.

## Audit and Review Outputs

Audit, review, plan, and analysis files go to `~/Documents/` with a descriptive name (e.g. `~/Documents/tempest-ui-audit.md`). Never write them inside the repo. They are personal working notes, not project artifacts, and should not show up in `git status`.

## i18n Key Edits

When adding or renaming an `fl!()` key, edit only `i18n/en/cosmic_ext_applet_tempest.ftl` and `i18n/en-US/cosmic_ext_applet_tempest.ftl`. The PreToolUse hook blocks other locales. Weblate fills in the rest on sync. Leave orphan strings in non-English locales for Weblate to reconcile.
