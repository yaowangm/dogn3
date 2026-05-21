---
name: code-review
description: Use when the user asks to review code quality, clean up code, remove unused code, refactor duplication, check performance, security, tests, comments, organization, or asks for a behavior-preserving code review pass.
---

# Code Review

Use this skill for rigorous code review and behavior-preserving cleanup.

## Core Rules

- Preserve existing behavior. Do not intentionally change product behavior, public APIs, data shape, routes, schema, or user-visible semantics.
- If a potential improvement requires a behavior change or has unclear tradeoffs, stop and ask the user before editing.
- Respect local repository rules: do not commit unless explicitly asked, never push, and do not access databases unless explicitly allowed.
- Prefer existing project patterns over new abstractions.
- Keep edits focused. Do not perform unrelated refactors.

## Review Checklist

- Unused code: remove dead imports, unused functions, obsolete files, redundant variables, and stale code paths when clearly safe.
- Duplication: refactor repeated logic only when it reduces real maintenance cost without obscuring intent.
- Performance: look for unnecessary repeated work, inefficient database/API calls, avoidable allocations, excessive DOM work, cache issues, and algorithmic hot spots.
- Security: check for XSS, unsafe HTML insertion, SQL injection, auth/session mistakes, path traversal, secret leakage, unsafe redirects, and insecure defaults.
- Organization: verify files, modules, names, and boundaries are tidy and consistent with the codebase.
- Comments: keep useful comments that explain non-obvious intent; add concise comments only where they reduce future confusion; remove stale/noisy comments.
- Tests: identify missing coverage for changed or risky behavior; add or update tests only when behavior-preserving and appropriate for the task.
- Validation: run the relevant formatter, type checks, linters, and tests available in the repo. If a check cannot be run, report why.

## Security Notes

- Treat direct HTML insertion as risky. Verify values are escaped or inserted as text, especially when rendering database or user-provided content.
- Prefer structured query APIs and parameter binding over string-built SQL.
- Do not log or expose secrets, credentials, tokens, or private environment values.

## Output Style

- For review-only requests, lead with findings ordered by severity and include file/line references.
- If no findings are found, say so clearly and mention residual test or validation gaps.
- For cleanup/edit requests, summarize behavior-preserving changes and validation commands run.
- Surface uncertain issues as questions instead of guessing.
