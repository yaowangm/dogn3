# Password Reset Design Draft

This document records the password-reset workflow and operational
requirements.

## Status

Implemented behind configuration.

Artifacts:

- `scripts/add_password_reset_tokens.sql`: creates the password reset token
  table and supporting indexes.
- `/login`: includes the reset-request form.
- `/reset_password?token={raw_token}`: confirms a new password.
- `POST /api/auth/password-reset/request`: creates and emails a reset token.
- `POST /api/auth/password-reset/confirm`: consumes a reset token and stores a
  new password.

## Mail Service Choice

Use Postfix on Ubuntu 24.04, not the Sendmail MTA.

Reasons:

- Postfix is the practical default MTA on Ubuntu and is simpler to operate
  than classic Sendmail.
- Postfix provides both a `sendmail`-compatible command at
  `/usr/sbin/sendmail` and a local SMTP listener. Standalone deployments may
  use the command interface; Docker deployments should use SMTP over
  `127.0.0.1:25` with `--network host`.
- For this application, the first deployment target should be send-only mail,
  not a full incoming mail server.

Recommended installation commands for the operator:

```bash
sudo apt update
sudo apt install postfix mailutils
```

During package configuration:

```text
General mail configuration type: Internet Site
System mail name: your-domain.com
```

Recommended send-only local configuration:

```bash
sudo postconf -e 'inet_interfaces = loopback-only'
sudo postconf -e 'mydestination = localhost'
sudo systemctl restart postfix
sudo systemctl enable postfix
```

Smoke test:

```bash
printf 'Subject: Dogn mail test\n\nHello from Postfix.\n' | /usr/sbin/sendmail your-email@example.com
```

SMTP smoke test for Docker-style configuration:

```bash
printf 'EHLO localhost\r\nMAIL FROM:<no-reply@example.com>\r\nRCPT TO:<your-email@example.com>\r\nDATA\r\nSubject: Dogn SMTP test\r\n\r\nHello from local SMTP.\r\n.\r\nQUIT\r\n' | nc 127.0.0.1 25
```

Check logs if mail does not arrive:

```bash
sudo journalctl -u postfix -n 100 --no-pager
```

For reliable external delivery in production, DNS and reputation work are
needed. At minimum configure SPF, DKIM, DMARC, and PTR/reverse DNS for the
sending host/domain.

## Configuration

Environment options:

| Option | Default | Purpose |
| --- | --- | --- |
| `PASSWORD_RESET_ENABLED` | `false` | Enables reset request and confirmation endpoints. Keep disabled until mail is verified. |
| `MAIL_DELIVERY` | `sendmail` | Mail backend: `sendmail` runs a local command; `smtp` sends through plain local SMTP. |
| `SENDMAIL_PATH` | `/usr/sbin/sendmail` | Local sendmail-compatible command, used only when `MAIL_DELIVERY=sendmail`. |
| `SMTP_HOST` | `127.0.0.1` | SMTP host, used only when `MAIL_DELIVERY=smtp`. For Docker, run with `--network host` so this reaches the host MTA. |
| `SMTP_PORT` | `25` | SMTP port, used only when `MAIL_DELIVERY=smtp`. |
| `MAIL_FROM` | none | Sender address, such as `no-reply@example.com`. Required when reset is enabled. |
| `PUBLIC_SITE_URL` | none | Public base URL used to build reset links. Required when reset is enabled. |
| `PASSWORD_RESET_TTL_SECONDS` | `1800` | Reset token lifetime, default 30 minutes. |

The application fails startup when reset is enabled but required mail/link
settings are missing.

Recommended Docker settings when the container is started with `--network host`
and Postfix accepts localhost SMTP:

```text
PASSWORD_RESET_ENABLED=true
MAIL_DELIVERY=smtp
SMTP_HOST=127.0.0.1
SMTP_PORT=25
MAIL_FROM=no-reply@example.com
PUBLIC_SITE_URL=https://example.com
```

No SMTP username or password is supported or required for this mode. It is
intended only for a trusted local MTA reachable through localhost.

## User Flow

1. User opens `/login`.
2. User clicks `Reset password`.
3. The login page switches to a password-reset request form asking for email.
4. User submits the email.
5. The API always returns a generic success message:

```text
If the email exists, a password reset message has been sent.
```

The generic response avoids revealing whether an email address belongs to an
account.

The email contains a one-time reset link:

```text
{PUBLIC_SITE_URL}/reset_password?token={raw_token}
```

The user opens the link, enters a new password, and submits it. The new
password uses the same policy as normal password changes:

- Length 8 to 30.
- Must include alphabet, number, and printable symbol.
- Printable ASCII only.

On success:

- Store the new password as `argon2id-v1`.
- Mark the reset token used.
- Invalidate active sessions for the user.
- Show a clear success message and let the user log in with the new password.

## API Design

Implemented endpoints:

```text
POST /api/auth/password-reset/request
POST /api/auth/password-reset/confirm
```

Request endpoint input:

```json
{
  "email": "user@example.com"
}
```

Request endpoint behavior:

- Requires the same-origin mutation request header.
- Normalizes the submitted email by trimming whitespace.
- Looks up active, non-frozen users by email.
- If exactly one eligible account is found, marks older unused tokens for that
  user as used, creates a fresh reset token row, and sends mail.
- If no account or multiple accounts match, returns the same generic success
  response without sending a reset link.
- Does not log raw tokens.

Confirm endpoint input:

```json
{
  "token": "raw-token-from-link",
  "new_password": "new password",
  "confirm_password": "new password"
}
```

Confirm endpoint behavior:

- Requires the same-origin mutation request header.
- Hashes the submitted raw token and looks up an unused, unexpired token row.
- Locks the token row during validation.
- Applies the existing password policy.
- Updates `user_info.password` and `user_info.password_scheme`.
- Sets `password_scheme = 'argon2id-v1'`.
- Sets `password_reset_token.used_at`.
- Invalidates active sessions for the user.

## Token Storage

Only a hash of the reset token is stored. The raw token appears only in the
email link and in the user's confirmation request.

Implemented table:

```text
password_reset_token
```

Columns:

- `id`: generated primary key.
- `user_id`: user receiving the reset token.
- `token_hash`: hash of the raw token.
- `created_at`: time the token was created.
- `expires_at`: time after which the token is invalid.
- `used_at`: set when the token is consumed.
- `request_ip`: optional IP address for audit/throttling.

`token_hash` is SHA-256 hex of a high-entropy 32-byte random token represented
as hex in the email URL. The raw token is generated with secure randomness and
is never stored.

## Security Requirements

- Never store raw reset tokens.
- Never log raw reset tokens.
- Use generic public responses to prevent email enumeration.
- Require same-origin mutation headers.
- Use one-time tokens.
- Enforce expiry.
- Invalidate active sessions after password reset.
- Store the new password directly as `argon2id-v1`; do not use the legacy
  `argon2id-md5-v1` path for reset passwords.
- Prefer plain text email. Do not include current password data.
- Keep reset links short-lived.

## Rate Limiting

Application-level rate limiting is implemented and documented in
`docs/RATE_LIMITING.md`.

Important design points:

- Production rate limiting depends on Redis.
- In-memory rate-limit fallback is allowed only for development.
- Reset requests are limited per normalized email and per direct client
  IP address.
- Invalid reset-token confirmations are limited per direct client IP
  address.
- Rate-limited reset requests must still return the generic public success
  message and must not create a token or send email.

Production rate limiting requires Redis. The in-memory fallback is for local
development only.

## Database Preparation

Run the SQL migration before enabling the feature:

```bash
psql dogn -f scripts/add_password_reset_tokens.sql
```

This command changes the real database schema by creating a new table and
indexes. It does not modify existing user rows or password data.

## Open Decisions

- Whether to require unique email addresses before enabling password reset.
  Current implementation handles ambiguous email matches by sending no reset
  link and returning the generic response.
- Final rate-limit policy.
