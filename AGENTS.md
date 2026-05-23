# Agent Instructions

- Commit only when the user explicitly asks for a commit.
- When the user asks for a commit, switch to the lightest available mode for
  the commit operation and switch back after it completes.
- Never push to any remote.
- Access the real database dogn only when the user explicitly allows database access.
- When real database access is allowed, read-only PostgreSQL queries are permitted.
- Feel free to access test database dogn_test when running test.
- Before running any query that modifies the database, ask again with a clear
  note that the action needs to change the database, and wait for explicit
  approval.
- Control the development server only through `./scripts/server.sh start`,
  `./scripts/server.sh stop`, `./scripts/server.sh restart`, or
  `./scripts/server.sh status`.
- Never start the server manually on another socket or port.
- When the user asks to "review code", always apply the `code-review` skill.
