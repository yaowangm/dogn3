# Authentication Rate Limiting Design Draft

This document records the proposed defense against exhaustive password
guessing and password-reset abuse. It is a design draft; implementation is not
started yet.

## Problem

The current authentication flows have bounded Argon2id concurrency, but a
remote client can still repeat login or reset-password requests indefinitely.
That leaves several risks:

- Brute-force password guessing through `/api/auth/login`.
- Broad username guessing by trying many names from one address.
- Password-reset mail abuse by repeatedly requesting reset emails.
- Reset-token guessing through `/api/auth/password-reset/confirm`.

The existing password-hash semaphore protects CPU capacity. It does not provide
account or IP retry limits and therefore is not sufficient by itself.

## Production Dependency

The production solution depends on Redis.

Reasons:

- Retry state must survive application restarts.
- Multiple application processes must share the same counters.
- Redis key expiry is a natural fit for fixed retry windows.
- The project already has optional Redis support, so the infrastructure choice
  is consistent with the existing architecture.

For production, rate limiting should be considered enabled only when Redis is
available. If Redis cannot be reached while production rate limiting is enabled,
the authentication endpoints should fail closed with a service-unavailable
response and a clear server log entry.

## Development Fallback

For development only, the application may fall back to in-memory retry
counters when Redis is unavailable.

The fallback is intentionally weaker:

- Counters disappear on server restart.
- Counters are local to one process.
- Counters do not protect a multi-process deployment.

This fallback exists only to keep local development simple. Documentation and
configuration should make clear that it is not acceptable for production.

## Configuration

Proposed environment options:

| Option | Default | Purpose |
| --- | --- | --- |
| `RATE_LIMIT_ENABLED` | `true` | Enables authentication-related retry limits. |
| `RATE_LIMIT_BACKEND` | `redis` | `redis` for production; `memory` allowed for development only. |
| `LOGIN_FAIL_WINDOW_SECONDS` | `900` | Login failure counting window, default 15 minutes. |
| `LOGIN_FAIL_MAX_PER_USER` | `5` | Failed login attempts allowed per normalized user name within the window. |
| `LOGIN_FAIL_MAX_PER_IP` | `30` | Failed login attempts allowed per direct client IP within the window. |
| `LOGIN_FAIL_LOCK_SECONDS` | `900` | Lockout duration after the login limit is exceeded. |
| `PASSWORD_RESET_WINDOW_SECONDS` | `3600` | Reset-request counting window, default 1 hour. |
| `PASSWORD_RESET_MAX_PER_EMAIL` | `3` | Reset requests allowed per normalized email within the window. |
| `PASSWORD_RESET_MAX_PER_IP` | `20` | Reset requests allowed per direct client IP within the window. |
| `PASSWORD_RESET_CONFIRM_WINDOW_SECONDS` | `900` | Invalid reset-token confirmation counting window. |
| `PASSWORD_RESET_CONFIRM_MAX_PER_IP` | `20` | Invalid reset confirmations allowed per direct client IP within the window. |

The client IP should initially be the direct TCP peer address. Do not trust
`X-Forwarded-For` or similar headers until a trusted-proxy deployment policy is
defined.

## Login Rate Limit

Login should be limited by both normalized user name and client IP.

Redis keys:

```text
rl:login:user:{normalized_name}
rl:login:ip:{ip}
lock:login:user:{normalized_name}
lock:login:ip:{ip}
```

Proposed flow:

1. Normalize the submitted user name the same way login lookup does.
2. Check user-name and IP lock keys before doing expensive password hashing.
3. If either lock exists, return `429 Too Many Requests` with a generic
   message:

   ```text
   Too many attempts. Try again later.
   ```

4. Process login normally.
5. On failed login, increment both counters using `INCR` and apply `EXPIRE`
   when the counter is first created.
6. If a counter exceeds its configured max, create the corresponding lock key
   with `LOGIN_FAIL_LOCK_SECONDS`.
7. On successful login, clear the user-name failure counter and user-name lock.
   The IP counter may remain so one successful account cannot erase broad
   guessing from the same address.

The public failure response must not reveal whether the user name exists.

## Password Reset Request Limit

Reset requests should be limited by normalized email and client IP.

Redis keys:

```text
rl:reset:email:{normalized_email}
rl:reset:ip:{ip}
lock:reset:email:{normalized_email}
lock:reset:ip:{ip}
```

Proposed flow:

1. Normalize the submitted email by trimming whitespace and lowercasing.
2. Check email and IP lock keys before account lookup and before token
   creation.
3. If rate-limited, skip token creation and mail sending.
4. Return the same generic public success message:

   ```text
   If the email exists, a password reset message has been sent.
   ```

5. If not limited, continue the existing reset-request flow.
6. Increment/reset counters for each request attempt. This limits both unknown
   emails and valid accounts.

The response must remain generic to avoid email enumeration and to avoid
revealing which addresses are throttled.

## Password Reset Confirmation Limit

Invalid reset-token confirmations should be limited by client IP.

Redis keys:

```text
rl:reset_confirm:ip:{ip}
lock:reset_confirm:ip:{ip}
```

Proposed flow:

1. Check the IP lock before token lookup.
2. If locked, return `429 Too Many Requests`.
3. On invalid or expired token, increment the IP counter.
4. If the counter exceeds the configured max, create the IP lock.
5. On valid token consumption, no counter clearing is required.

Returning `429` here is acceptable because the endpoint already requires a
token and does not expose account existence. The invalid-token response remains
appropriate before the limit is reached.

## Redis Operations

For this stage, simple Redis commands are sufficient:

```text
INCR key
EXPIRE key window_seconds
SET lock_key 1 EX lock_seconds
DEL key
```

The implementation should set `EXPIRE` when the counter value becomes `1`.
Later, these operations can be moved into a Lua script if strict atomicity is
needed under high concurrency.

## Backend Structure

Add a small `rate_limit` module responsible for:

- Loading rate-limit configuration.
- Choosing Redis or development memory backend.
- Normalizing key material where appropriate.
- Checking locks.
- Recording failed or attempted operations.
- Clearing user-specific login failures after successful login.

Authentication routes should call this module rather than manipulating Redis
keys directly.

## Logging

Logs should be useful without exposing secrets:

- Log when a rate limit blocks a request.
- Include the rate-limit bucket type, such as `login_user`, `login_ip`,
  `reset_email`, or `reset_ip`.
- Include user id only after an authenticated or successful lookup path has
  already resolved it.
- Do not log raw passwords, reset tokens, or full reset URLs.
- Avoid logging email addresses unless they are hashed or otherwise minimized.

## Tests

Required coverage:

- Login is blocked after the configured per-user failure limit.
- Login is blocked after the configured per-IP failure limit.
- Successful login clears the user-name failure counter.
- Reset request rate limit returns the generic success message and sends no
  email.
- Unknown email reset requests are also counted.
- Duplicate-email reset requests still return the generic message and send no
  email.
- Invalid reset-token confirmations are blocked after the configured IP limit.
- Redis-backed limits use shared state across app instances.
- Development memory fallback works locally but is documented as non-production.
- Redis-unavailable production behavior fails closed.

## Open Implementation Notes

- The exact Redis connection should reuse the existing optional Redis
  configuration where possible.
- The rate-limit module should make it difficult to accidentally enable the
  memory fallback in production.
- If a trusted reverse proxy is introduced, client IP extraction must be
  revisited before rate limits are considered reliable.
