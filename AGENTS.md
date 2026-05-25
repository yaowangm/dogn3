# Agent Instructions

- Commit only when the user explicitly asks for a commit.
- When the user asks for a commit, commit the current code immediately with a
  proper descriptive message, without running checks or doing other work first.
- When the user asks for operations related to github e.g. "commit", switch to the
  lightest available mode for the commit operation and switch back after it completes.
- Never push to any remote.
- Access the real database dogn only when the user explicitly allows database access.
- When real database access is allowed, read-only PostgreSQL queries are permitted.
- Feel free to access test database dogn_test when running test.
- A script whose execution against the real database was explicitly approved
  previously may be run again without requesting approval again, including
  the database access or changes already covered by that approval. Do not
  broaden the script's approved target or behavior.
- Before running any query that modifies the database, ask again with a clear
  note that the action needs to change the database, and wait for explicit
  approval, unless it is a rerun covered by the approved-script exception
  above.
- Control the development server only through `./scripts/server.sh start`,
  `./scripts/server.sh stop`, `./scripts/server.sh restart`, or
  `./scripts/server.sh status`.
- Never start the server manually on another socket or port.
- When the user asks to "review code", always apply the `code-review` skill.
