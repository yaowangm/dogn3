# Page Design Draft

This document records page-level design decisions. It starts with the default
page and should grow as future pages are designed.

## Page Design Principles

All pages should follow the project frontend direction:

- Use HTML5, CSS3, native Web Components, and REST-style JSON APIs.
- Keep dynamic content out of server-rendered HTML. Fetch dynamic page data from
  backend JSON endpoints.
- Keep one stable API usable by the browser UI and future clients.
- Prefer semantic HTML, keyboard-accessible controls, visible focus states, and
  high-contrast colors.
- Use simple line-drawing SVG icons with consistent stroke width, sizing, and
  alignment.
- Keep pages scan-friendly with clear hierarchy, whitespace, and restrained
  visual effects.
- Design primarily for desktop while keeping mobile layouts fully functional.
- Prefer scrolling over unnecessary clicking for content consumption.
- Avoid pop-up-heavy flows, scroll hijacking, and excessive infinite scrolling.
- Render interface labels through the planned internationalization framework
  described in `docs/I18N.md`, with English fallback and Simplified Chinese
  support selected by browser language.

## Page Inventory

| Page | Browser route | JSON API used | Purpose | Primary operations |
| --- | --- | --- | --- | --- |
| Portal / default | `/` | `GET /api/home` | Overview of recent posts, users, and boards. | Open posts or users in new windows; enter boards; open header menus or login. |
| Login and reset password | `/login`, `/reset_password?token={token}` | `GET /api/auth/session`, `POST /api/auth/login`, `POST /api/auth/logout`, `POST /api/auth/password-reset/request`, `POST /api/auth/password-reset/confirm` | Establish or end an authenticated session; request and complete email-based password reset. | Submit credentials; return to the originating local page after login/logout; request a reset email; set a new password from a reset token. |
| Board | `/board/{board_id}` | `GET /api/boards/{board_id}?page={page}` | Read paged post trees in one board. | Page through results; open posts or authors in new windows; begin a new root post after login. |
| Post | `/post/{post_id}` | `GET /api/posts/{post_id}` | Read one full post with tree context. | Enter board; switch to list view; open print view; set or unset a root-post favorite after login; owners/administrators open editor. |
| Post editor | `/post_upd?board_id={board_id}`, `/post_upd?post_id={post_id}`, or `/post_upd?reply_to={post_id}` | `GET /api/post_upd?...`, `POST /api/post_upd` | Add a root post, reply to a post, or update an existing post. | Add or reply as an authenticated user; update roots as their owner/admin and replies as admin only. |
| Post list | `/post_list/{post_id}` | `GET /api/post_lists/{post_id}` | Read a complete discussion tree as full post cards. | Navigate full posts and compact tree; focus the selected reply. |
| Post print | `/post_print/{post_id}` | `GET /api/post_prints/{post_id}` | Minimal browser-printable post representation. | Use browser print; follow safe related links. |
| Search | `/search` | `GET /api/search/posts?...` | Authenticated lexical post search. | Search all visible posts by separate subject/content/user-name keywords, date ranges, type, image attachment flag, id order, and page. |
| User | `/user/{user_id}` | `GET /api/users/{user_id}?activity={activity}&page={page}`, profile/password/statistics mutation endpoints, and `POST /api/users/{user_id}/role` | View profile status and activity. | Select activity/page; open posts/users; update permitted profile fields; change password; recalculate statistics; administrators set roles. |
| User list | `/user_list` | `GET /api/users?query={query}&role={role}&order={order}&page={page}` | Administrator-only member directory. | Search names/email; filter roles; sort by id; page results; open profiles. |
| User add | `/user_add` | `GET /api/users?query={query}` and `POST /api/users` | Administrator-only account creation. | Enter identity and initial password; optionally select an introducer; open the created profile. |
| Site manager | `/site_mgr` | `GET /api/site_manager`, category/board mutation endpoints, and `POST /api/site_manager/boards/statistics/recalculate` | Administrator-only board/category maintenance. | Create/edit/delete empty taxonomy nodes, manage board masters, and recalculate all board statistics. |

Supporting routes that are not browser pages:

| Route | Purpose | Authorization-sensitive behavior |
| --- | --- | --- |
| `GET /api/health` | Report service/database/cache health. | Not used for user navigation. |
| `GET /images/{*path}` | Serve approved local post image attachments. | Encrypted-only images require login; deleted/unknown-only images fail closed. |
| `POST /api/posts/{post_id}/delete` | Soft-delete a post or root tree. | An author may delete their childless root; a board master of its board or administrator may delete any post. Deleting a root sets every stored tree post to `state = 2`. |
| `POST /api/posts/{post_id}/favorite` | Set or unset the root post in the current user's favorites. | Requires login and a verified mutation request; accepts visible root posts only; accepts a desired `favorited` state and never creates duplicate relations. |

## Routing And Interaction Model

All interactive pages except print use the shared HTML shell and the
`dogn-app-shell` native Web Component. The component recognizes the browser
path, requests its JSON endpoint, then renders dynamic content. The print page
uses a minimal shell without the shared header and footer.

The server injects page-specific `<title>`, canonical URL, description, and
Open Graph metadata into the HTML shell before JavaScript runs so crawlers and
link-preview robots can read useful page information. Major shareable pages
include portal, board, post, post-list, print-post, and user pages. `og:image`
uses the configured site icon path. When `PUBLIC_SITE_URL` is configured, it is
used to generate absolute `og:url`, canonical URL, and image URL values;
otherwise route-local paths are used. Encrypted posts may expose title and
public metadata but never expose protected body content in the description.

| Interaction | Destination or API | Window behavior | Current logic |
| --- | --- | --- | --- |
| Brand icon/site name | Portal/board popup menu | Overlay on current page | Toggle menu; outside click or `Escape` closes it; open menu masks content below header. |
| Portal menu item | `/` | Current window | Return to portal overview. |
| Board name | `/board/{board_id}` | Current window | Enter board from the menu, portal card, board intro, or post controller. |
| Post title in portal or board tree | `/post/{post_id}` | New window | Keep list-reading context available. |
| Post subject in full post-list card | `/post/{post_id}` | Current window | Move from aggregate tree reading to selected post detail. |
| Interactive user name | `/user/{user_id}` | New window | Applies to portal users, post authors, point awards, and introducer metadata. |
| Header `Profile` command | `/user/{logged_in_user_id}` | Current window | Enter own profile from account menu. |
| Header `Add user` command | `/user_add` | Current window | Shown to administrators only; user creation is independently authorized by the API. |
| Header `User list` command | `/user_list` | Current window | Shown to administrators only; the API independently enforces administrator access. |
| Header `Site manager` command | `/site_mgr` | Current window | Shown to administrators after `Add user`; API/mutations enforce administrator access. |
| Login link | `/login?return_to={local_page}` | Current window | Carry only validated application-local return navigation. |
| Successful login or logout | Prior local page, otherwise `/` | Current window | Restore reading context in new authentication state. |
| List-view tool | `/post_list/{post_id}` | Current window | Display full post tree. |
| Print tool | `/post_print/{post_id}` | New window | Open minimal printable post page. |
| Board `Add post` action | `/post_upd?board_id={board_id}` | Current window | Anonymous selection goes through login; logged-in users open a root-post editor. |
| Post update tool | `/post_upd?post_id={post_id}` | Current window | For roots, rendered to owner/admin; for replies, rendered to admin only. The backend enforces the same rule. |
| Safe external resource pill | Valid `http`/`https` URL | New window | Uses `noopener noreferrer`; unsafe targets are omitted. |

Current mutation boundary:

- Login, logout, password reset, authorized password change, and authorized
  user-statistics recalculation are implemented state-changing UI operations.
- Root-post creation and owner/administrator post editing are implemented via
  `/post_upd`.
- Favorite changes and moderation workflows are implemented.
- A visible or reserved control is not backend authorization; each future
  mutation endpoint must enforce its privilege policy independently.

## Shared UI And State Rules

### Visual Language

- Page background uses the darkest of the light neutral surface levels.
- Section headers use a slightly darker neutral background than item bodies;
  unselected list items remain white and hover with a restrained darker fill.
- Individual content cards and tree cards use thin linear borders with an
  `8px` maximum corner radius. Only popup menus use a blurred shadow.
- Line-drawing SVG icons are used consistently for page sections, post types,
  resources, metadata, operations, and account navigation.
- Post-type icons communicate type with foreground color: announcement
  orange, normal blue, original red, and forward green.
- Pills are used for compact quantitative metrics and status/resource
  indicators; status icons do not appear when there is no corresponding state.
- Layout is desktop-oriented with responsive single-column behavior and
  constrained controls on small viewports.

### Typography

The site uses local system fonts only. It must not depend on Google Fonts or
other downloaded web fonts because some deployment and browsing environments
cannot reliably access external font providers.

The sans-serif stack is optimized for mixed CJK and English text:

```css
system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI",
"Microsoft YaHei UI", "Microsoft YaHei", "PingFang SC",
"Hiragino Sans GB", "Noto Sans CJK SC", "Noto Sans SC",
"Source Han Sans SC", "WenQuanYi Micro Hei", Roboto,
"Droid Sans Fallback", "Helvetica Neue", Arial, sans-serif
```

Ordering rules:

- `system-ui`, Apple system UI fonts, and `Segoe UI` keep English UI text
  native on macOS, iOS, and Windows.
- `Microsoft YaHei UI` / `Microsoft YaHei` cover Simplified Chinese on
  Windows.
- `PingFang SC` and `Hiragino Sans GB` cover Chinese on macOS and iOS.
- `Noto Sans CJK SC`, `Noto Sans SC`, `Source Han Sans SC`, and
  `WenQuanYi Micro Hei` cover common Linux distributions.
- `Roboto` and `Droid Sans Fallback` cover Android Latin and older Android CJK
  fallback behavior.
- `Helvetica Neue`, `Arial`, and generic `sans-serif` remain final fallbacks.

Code blocks use a local monospace stack with CJK-capable fallbacks:

```css
ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono",
"Noto Sans Mono CJK SC", "Source Han Mono SC", monospace
```

### Dynamic Data And Failure States

- Browser page HTML is a shell; dynamic forum content is loaded from JSON
  endpoints through Ajax.
- Each interactive data page begins with loading text or loading sections.
- Endpoint failure leaves the shell visible and replaces page content with a
  neutral failure state.
- Login-required, administrator-required, unavailable-resource, and API failure
  states use the shared message panel: visible padding, warning/error icon,
  concise title, explanatory text, and a clear action link when recovery is
  possible.
- Missing posts and users use an unavailable/not-found presentation rather
  than exposing backend details.
- Shared header board-menu contents are provided by page JSON responses where
  navigation context is required.

### Visibility And Safety Rules

- APIs list only posts in normal or encrypted states; deleted and unknown
  states fail closed.
- Anonymous users may see metadata for encrypted posts, but not protected body
  content or protected resource locations.
- Post, session-dependent, board, and user responses are sent with
  `Cache-Control: no-store`; the portal has optional Redis caching with
  public/authenticated variants.
- All dynamic text is escaped before insertion into HTML.
- Resource URLs are rendered only after safe/local URL validation; external
  navigation opens without giving the new page an opener reference.

## Shared Page Shell

The shared shell is currently implemented by the `dogn-app-shell` Web
Component.

Every page should eventually share these regions:

- Header.
- Main content.
- Footer.

### Header

The header is sticky and appears at the top of the viewport.

Current behavior:

- Left side contains the site logo and site name.
- The logo and site name are one menu button.
- Activating the button opens the portal/board menu.
- Right side shows `login` when no user is logged in.
- When a valid session exists, the right side shows a pill button containing
  the user icon and name; it opens a menu containing profile, search, and
  logout actions.

The site name is read from the backend response and falls back to `Dogn`.
The site logo has one source of truth: the static SVG asset at
`/assets/favicon.svg`. The favicon link, Open Graph image, header brand, login
panel, and print metadata all reference that same file instead of duplicating
the SVG markup inline.

### Portal/Board Menu

The portal/board menu opens from the header brand button.

Current content:

- A `Portal` link to `/`.
- Board links grouped by category.
- Category headings use the same line-drawing board icon style as the page.

Current interaction:

- Clicking the brand button toggles the menu.
- Clicking outside the menu closes it.
- Pressing `Escape` closes it and focuses the brand button.
- Opening the menu darkens the page below the header with a subtle mask.

### Account Menu

For anonymous visitors, the header shows `login`, linking to the login page
with the current local route as a return destination.

For a logged-in visitor, the user icon-and-name pill opens a dropdown:

- `Profile` opens the logged-in user's page in the current window.
- `Add user` appears immediately after `Profile` for administrators and opens
  account creation.
- `Site manager` appears after `Add user` for administrators and
  opens board/category maintenance.
- `User list` appears for administrators after `Site manager` and opens the
  administrator-only directory.
- `Search` opens the authenticated post-search page.
- `Exit` calls the logout API and returns to the current local page in
  anonymous state.

Clicking outside the account dropdown closes it.

### Footer

The footer appears at the bottom of the page shell.

Current content:

- Site name.
- Short technology/site description.
- Copyright line with the current year.

Footer content is basic for now and should be refined later when real site
identity, links, policies, and contact information are known.

## Default Page

Route:

```text
/
```

Static shell:

```text
static/index.html
```

Web Component:

```text
static/js/app.js
```

Backend page route:

```text
GET /
```

Backend data route:

```text
GET /api/home
```

### Purpose

The default page is the forum portal. It gives a compact overview of recent
activity, important post categories, active users, and available boards.

It is not a marketing landing page. It is an operational entry point for forum
reading and navigation.

### Page Structure

The default page contains:

- Shared header.
- Intro section.
- Dashboard section.
- Shared footer.

### Intro Section

The intro section sits above the dashboard.

Current content:

- Eyebrow: `Forum`.
- Main heading: configured site name.
- Supporting sentence: recent discussions, original posts, forwards, users, and
  boards.

Current styling:

- Bordered section.
- White background over a slightly darker page background.
- Restrained typography and spacing.

### Dashboard Sections

The dashboard is a responsive card grid.

Current cards:

- Recent announcement posts.
- Recent root posts.
- Recent original posts.
- Recent forward posts.
- New users.
- Top point users.
- Boards.

Each card has:

- A header with a line-drawing SVG icon.
- A title.
- A list area.
- Empty/loading/error state where applicable.

Cards use subtle borders and level-based backgrounds rather than heavy shadows.
Items are separated by thin divider lines. Hover states use a darker background
instead of border changes.

### Post Cards

Post cards are used by:

- Recent announcement posts.
- Recent root posts.
- Recent original posts.
- Recent forward posts.

Each post item includes:

- Post type icon.
- Post title.
- Optional status icon bar.
- Board name.
- Author display name or fallback user id.
- Post time.
- Reply/tree count when present; for root posts this is the inclusive stored
  post-tree count represented by `post.reply_count`.
- View count.
- Point count.

Post metadata uses small line-drawing icons with accessible labels and hover
titles. The entire item is visually treated as clickable through the title
link overlay; portal post links open the post page in a new browser tab.
Author-name links open user pages in a new browser tab.

Post type colors:

- Normal: blue.
- Original: red.
- Forward: green.
- Announcement: orange.

Icon style:

- Blank/light background.
- Colored foreground.
- Border uses the same color as the icon foreground.

Status bar:

- Shown only when needed.
- Displays a link icon when `post.link_url` contains a safe related URL.
- Displays image attachment icon when `post.image_url` is present.
- Displays encrypted icon when `post.state = 1`.
- Uses a blank background and black icon/border treatment.

Deleted posts and posts with unsupported visibility states are excluded by the
backend query.
Encrypted post cards retain their visible metadata and attachment indicators;
protected resource URLs are not included for anonymous visitors.

### User Cards

User cards are used by:

- New users.
- Top point users.

Each user item includes:

- User icon.
- User name.
- Joined date.
- Two metric pills at the right:
  - Post count.
  - Point count.

Metric pills use a pill/capsule shape so large numbers fit better than a fixed
circle.
Selecting a user name opens `/user/{user_id}` in a new browser tab.

### Board Card

The Boards card is a full-width dashboard section.

Boards are grouped by category.

Each category includes:

- Category header.
- Board grid.

Each board item includes:

- Board icon.
- Board name.
- Board comment.
- Two metric pills:
  - Post count.
  - Root count.

Selecting a board name opens `/board/{board_id}` in the current window.

### Loading And Error States

Before `/api/home` returns, dashboard cards show loading states.

If the JSON API fails:

- The static page shell remains visible.
- The dashboard is replaced by an error section.
- The browser console receives the original error.

### Data API

Endpoint:

```text
GET /api/home
```

Response shape:

```text
site_name
recent_announcement_posts
recent_root_posts
recent_original_posts
recent_forward_posts
new_users
top_point_users
boards
```

The response is client-oriented and should not be treated as a direct database
table mirror.

### Backend Query Behavior

Post lists:

- Announcement posts filter by post type `3`.
- Original posts filter by post type `1`.
- Forward posts filter by post type `2`.
- Root posts filter by `parent_id` empty or `0`.
- Only normal and encrypted posts are listed (`state IN (0, 1)`); deleted or
  unknown states are excluded.
- Each list is limited to 10 rows.

Users:

- New users sort by newest id first.
- Top point users sort by points descending, then id ascending.
- Each list is limited to 10 rows.

Boards:

- Boards join category.
- Boards sort by category order, board order, then board id.

### Cache Behavior

`/api/home` uses optional Redis read-through caching.

Cache key:

```text
api:home:v4:generation
api:home:v4:public:{generation}
api:home:v4:authenticated:{generation}
```

Behavior:

- Public and authenticated variants separate protected resource locations for
  encrypted post summaries.
- If cache is enabled and contains a valid response, return cached JSON.
- On cache miss, read PostgreSQL and write the response to Redis.
- Redis read/write runtime errors are logged and fall back to PostgreSQL.
- If cache is disabled, PostgreSQL is always used.

Current invalidation:

- Authorized user-statistics recalculation advances the home cache generation
  after updating `user_info`, because user cards contain `post_count`.
- In-flight responses may finish writing to an older generation, but they
  cannot be read after the new generation becomes current.
- If generation advancement fails, this application process stops using the
  home cache so it does not return potentially stale statistics.
- Other cached-data writes are not implemented yet and retain TTL-only
  invalidation via `REDIS_DEFAULT_TTL_SECONDS`.

Future invalidation:

- Post, user, board, and category writes affecting portal data should advance
  the home cache generation after successful database transactions.

### Accessibility Notes

Current accessibility choices:

- Header navigation uses semantic `header` and `nav`.
- Main content uses `main`.
- Cards use section headings with `aria-labelledby`.
- Menus expose `aria-haspopup`, `aria-expanded`, and `role="menu"` where
  applicable.
- SVG icons are decorative and marked `aria-hidden="true"`.
- A screen-reader-only utility exists for hidden labels.
- Focus-visible styles are defined globally.

Known future work:

- Add frontend tests or manual accessibility checks when page interactions grow.

### Security Notes

Dynamic string values rendered through `innerHTML` must be escaped.

Current frontend helpers:

- `escapeHtml` escapes dynamic text.
- Post titles, metadata, user names, board names, board comments, category
  names, site names, and metric labels/values should pass through escaping or
  text assignment.

Backend queries use structured SQLx APIs and parameter binding for dynamic
query values.

### Open Questions

- Final site identity, footer links, and copyright wording.
- Durable authentication session persistence.
- Whether `/api/home` should use more granular cache keys later.
- Whether the default page should include pagination or only fixed overview
  lists.

## Search Page

Route:

```text
/search
```

Backend route:

```text
GET /api/search/posts
```

The search page is available only to logged-in users. Anonymous users who open
`/search` receive the normal page shell, but the JSON API returns
`401 authentication_required` and the browser shows a login prompt that returns
to `/search` after authentication.

The current version is lexical search, not vector search. It uses PGroonga
inside PostgreSQL for Chinese and multilingual full-text matching on subject,
content, and user-name fields. Vector search with `pgvector` is intentionally
deferred.

Query parameters:

| Parameter | Meaning |
| --- | --- |
| `subject` | Keyword searched only in `post.subject`. |
| `content` | Keyword searched only in `post.content`. |
| `user_name` | Keyword searched only in `post.user_name`. |
| `created_from`, `created_to` | Inclusive creation-date range from date inputs. |
| `replied_from`, `replied_to` | Inclusive reply-date range from date inputs. |
| `post_type` | Optional post type: `0` normal, `1` original, `2` forward, `3` announcement. |
| `has_image` | When `true`, require a non-empty `post.image_url`. |
| `order` | `id_desc` by default, or `id_asc`. |
| `page`, `page_size` | Paged result control; page size is clamped by the API. |

The API searches visible posts that can be opened by the post page, meaning
`post.state IN (0, 1)` and `post.board_id` must match an existing board.
Deleted posts and orphaned legacy posts whose board no longer exists are
excluded. Because login is required, encrypted posts can appear with normal
metadata, attachment flags, related links, and image paths.

The result list uses the existing post-card style and opens post links in a new
window. Each result includes post type/status icons, title, post metadata,
and board link metadata. Content excerpts are intentionally not shown.

Production databases should install PGroonga and run
`scripts/add_post_pgroonga_search_indexes.sql` to add search-supporting indexes.
The search API depends on PGroonga being available in the database.

## Login And Reset Password Pages

Route:

```text
/login
/reset_password?token={token}
```

Backend API routes:

```text
POST /api/auth/login
GET  /api/auth/session
POST /api/auth/logout
POST /api/auth/password-reset/request
POST /api/auth/password-reset/confirm
```

### Purpose

The login page authenticates an existing forum user using the migrated
credential representation and establishes an opaque server-managed session.
The reset-password flow lets a user request an email reset link and set a new
direct `argon2id-v1` password without exposing legacy password material.

### Page Structure

The login page contains:

- Shared header and footer.
- A focused login card.
- Labeled user-name input.
- Labeled password input.
- Login submit button.
- Reset password button.
- Hidden reset-request form asking for email address.
- Generic invalid-credentials error state.

The login form submits JSON through Ajax. Credentials are never placed in a
URL. The login link carries only a local `return_to` page destination. On
success the browser receives an `HttpOnly` session cookie and returns to the
page that opened login, falling back to the portal page when no valid previous
page is available. Once the session is detected, the shared header displays a
user icon-and-name menu trigger rather than the login link. Logout uses a POST
API action and reloads the current page in anonymous state, with the same
portal fallback if no valid local page is available.

The reset-password confirmation route uses the same shared shell but renders a
focused card from the `token` query parameter. If the token is missing, the
page shows a neutral missing-link state. If the token is present, the page asks
for a new password and confirmation. On success it shows a marked success
message and hides the password fields to avoid repeated submission.

The login panel shows the site logo in a left-side brand area beside the input
form on desktop. On narrow screens, the logo stacks above the form with a
simple divider so the form remains readable.

### Operation Logic

- Page initialization calls `GET /api/auth/session` to render the login link
  or authenticated account menu.
- Form submission calls `POST /api/auth/login` with JSON `name` and
  `password`; credentials are never placed in a URL.
- Successful login issues the opaque session cookie and navigates to a valid
  local `return_to` route or `/`.
- Successful login updates the account's last-login timestamp, direct client
  IP address, and login counter. Failed attempts do not update those fields.
- A failed attempt for a recognized user name updates that account's login
  error timestamp and counter. Frozen accounts receive a clear frozen-account
  message; invalid credentials, unknown users, and unsupported/unmigrated
  credentials receive the generic failure message.
- Header logout calls `POST /api/auth/logout`, clears the live session cookie,
  and reloads the prior local page as an anonymous visitor.
- Login page reset-request submission calls
  `POST /api/auth/password-reset/request` with an email address and always
  shows the same generic success message for public success cases.
- Reset confirmation calls `POST /api/auth/password-reset/confirm` with the
  raw token, new password, and confirmation. A valid token updates the stored
  credential to `argon2id-v1`, marks the token used, and invalidates existing
  sessions for the user.

### Security Notes

- Frozen accounts receive a specific visible login failure message. Invalid,
  unknown, and unmigrated accounts receive the generic failure message.
- Frozen accounts are identified by `user_info.level = 0`;
  `user_info.state` does not control login eligibility.
- Password inputs are processed only by the authentication API.
- Reset links contain a raw one-time token; only its SHA-256 hash is stored in
  the database.
- Reset emails are sent through the configured mail backend when
  `PASSWORD_RESET_ENABLED=true`: either the local sendmail-compatible command
  or plain local SMTP for Docker host-network deployments.
- Session API responses are marked non-cacheable.
- The initial session store is in application memory, so sessions expire or
  disappear on server restart; persistent sessions remain a future design
  decision.

## Board Page

Route:

```text
/board/{board_id}
```

Backend page route:

```text
GET /board/{board_id}
```

Backend data route:

```text
GET /api/boards/{board_id}
```

### Purpose

The board page shows board metadata and a paged list of post trees inside one
board.

Browser title:

```text
{board name} - {site name}
```

### Page Structure

The board page contains:

- Shared header.
- Intro section used as the board info card.
- Optional `Recent announcement` card.
- Top pager controller with an `Add post` action at the right.
- Direct post tree cards.
- Pager controller.
- Shared footer.

Both pager controllers include the same page navigation. The top pager also
shows an `Add post` action at the right. Logged-in users enter
`/post_upd?board_id={board_id}`; anonymous users enter login first with this
local destination retained.

If the board has at least one visible announcement post (`post.type = 3`), the
page shows a `Recent announcement` card before the first pager. The card
displays only the most recent announcement post in that board, ordered by
descending post id. It uses the same compact board-post item component as post
tree cards and includes title and metadata only, not post body content. If no
visible announcement exists, the card is omitted.

### Board Info In Intro

The board page does not render a separate board info card below the intro. The
intro section itself is the board info surface, avoiding duplicated title and
description cards.

The intro includes:

- Board name, linked to `/board/{board_id}`.
- Board comment.
- Category name.
- Post count.
- Root/thread count.
- Ordered board master users from `board_master`, joined to `user_info`.

If no board master relationships are present, the UI shows a neutral fallback.

### Pager Controller

The pager controller includes:

- First page.
- Previous page.
- Current page index.
- Total page count.
- Next page.
- Last page.

Disabled pager links are visually muted and do not accept pointer events.

### Post Tree List

The page displays root post trees directly between the two pager controllers.
There is no extra wrapper card titled `Post trees`. Each tree is its own card,
and each tree card contains multiple post items ordered by the database tree
traversal order. Selecting a post item opens its post page in a new browser
tab.

Each post item includes:

- Post type icon.
- Post title.
- Author name or fallback user id.
- Post time.
- Post size.
- Access/view count.
- Point count for root posts only.
- Status bar.
- Tree-post count for root posts only, sourced from `post.reply_count`; this
  includes the root itself and all stored tree members.
- Latest reply time for root posts only.

The status bar is placed directly after the post title.
Metadata values use compact line-drawing icons rather than repeated text labels;
icons still expose accessible labels for assistive technology.
Author-name links open user pages in a new browser tab.

Each post item is indented according to `post.level`. The root has level `0`;
direct replies have level `1`; deeper replies continue to indent. The UI caps
extreme indentation so very deep trees remain readable.

### Data API

Endpoint:

```text
GET /api/boards/{board_id}?page=1&page_size=50
```

`page_size` is optional. It controls the number of visible post items per page.
When omitted, the backend uses `BOARD_PAGE_SIZE`, which defaults to `50`.

Response shape:

```text
site_name
board
pager
trees
boards
```

`boards` is included so the shared header board menu can be populated on the
board page without making a second request.

### Backend Query Behavior

The API fetches board metadata separately from posts.

Visible posts are fetched in one SQL query:

- Include only normal and encrypted posts (`state IN (0, 1)`).
- Order trees by newest root first.
- Order posts inside each tree by `order_num`.
- Apply `LIMIT` and `OFFSET` to the ordered post rows.
- Group the returned rows into tree cards by `root_id`.

This follows the database design rule that many post trees can be displayed in
correct depth-first order by sorting with root order and `order_num`.
The root-post count is stored separately from page visibility filtering: it is
the inclusive stored tree count and can therefore exceed the number of
currently rendered posts when a tree contains non-visible states.

### Operation Logic

- Top and bottom pagers build the current board route with a changed page
  query value; boundary controls are styled and disabled appropriately.
- Post title selection opens `/post/{post_id}` in a new window.
- Author selection opens `/user/{user_id}` in a new window.
- The intro title links to the canonical current board route.
- The `Add post` command enters the authenticated root-post editor; no board
  mutation occurs until the editor form is submitted.
- No subscription or moderation action is implemented on this page.

### Cache Behavior

The board page is not cached yet.

Future cache keys should include board id and page number, for example:

```text
api:board:{board_id}:page:{page}:v1
```

Implemented post creation and editing invalidate the default-page cache key.
Any future board-page caching must also be invalidated by those mutations.

### Open Questions

- Final page size and whether users can choose it.
- Whether request-provided `page_size` should remain public or become internal
  only.
- Whether board page data should be cached before write flows exist.
- Whether board masters should eventually link to user profile pages.

## Post Editor Page

Routes:

```text
/post_upd?board_id={board_id}
/post_upd?post_id={post_id}
/post_upd?reply_to={post_id}
```

Backend routes:

```text
GET /api/post_upd?board_id={board_id}
GET /api/post_upd?post_id={post_id}
GET /api/post_upd?reply_to={post_id}
POST /api/post_upd
```

### Purpose And Scope

The editor is one reusable page for creating a new root post in a board,
replying to an existing post, and updating an existing post.

The shared mutation endpoint decides the operation from the submitted target
fields:

- `board_id` only: create a new root post in that board.
- `parent_id` only: create a reply to the selected post.
- `post_id` only: update an existing post.

Exactly one target kind is accepted. Supplying multiple targets or no target is
rejected before any post data is changed.

### Page Structure

- Shared header and footer.
- Controller strip linking to the target board.
- Editor card with subject, encrypted checkbox, and body. Root creation and
  reply modes provide image file upload; update mode never changes an attached
  image.
- Root creation and root update present iconed type choices. Root creation
  also shows the configured daily point-award rule beside those choices. Reply
  creation and non-root update omit the type choices because non-root posts are
  always normal.
- Reply mode identifies its target as `Reply to: {post subject}` and does not
  expose a type selector because replies are always normal posts.
- Reply mode exposes `Points to author` only when replying directly to a root
  post. It shows the current user's available points and limits browser input
  to `0` or `1` through the smaller of the user's balance and
  `POST_REPLY_MAX_POINTS` (default `100`). Zero makes no transfer. Replies to
  non-root posts must submit zero points.
- Primary publish/save command and cancel navigation.

The editor provides iconed Normal, Original, Forward, and Announce type
choices. Encryption is a lock-icon checkbox. Deletion is not presented as an
editing mode.

### Authorization And Mutation Logic

- A live login session is required for new posts.
- Creating a new post through `board_id` always creates a root post in that
  board. It may choose Normal, Original, Forward, or Announce type, may be
  marked encrypted, may include body text, and may upload one initial image.
  It cannot transfer points through the submitted reply-points field, but it
  may earn the author an automatic root-post activity award.
- A live login session is required for replies; any active logged-in user may
  reply to a visible post while its tree root is no older than
  `POST_REPLY_MAX_AGE_DAYS`, which defaults to 10 days.
- Positive reply point transfer is available only when the direct reply target
  is a root post. Replying to a non-root post may still create the reply, but
  it cannot transfer points. Replying to the user's own root post may still
  create the reply, but it cannot transfer points.
- A positive reply point transfer must not exceed `POST_REPLY_MAX_POINTS` or
  the replying user's current balance.
- A root post may be updated by its owner or an administrator, unless the post
  has ever been selected as a user signature. Signature-history posts are
  locked against non-admin updates even when they are no longer the latest
  signature. A non-root post may be updated only by an administrator.
- API endpoints enforce permissions independently of visible UI controls and
  require the same-origin mutation header on save.
- Subject length is limited by `POST_SUBJECT_MAX_LENGTH`, defaulting to 50
  characters. Body content is limited by `POST_CONTENT_MAX_BYTES`, defaulting
  to 131072 UTF-8 bytes (128 KB). The editor reflects the subject limit and
  prechecks body bytes; the backend rechecks both values.
- The editor stores `post.content_format` with each save. `0` is legacy plain
  text and is the default for existing migrated posts. `1` is Markdown and is
  rendered on the client after sanitization. The format is selected when the
  post is created or replied to and cannot be changed by later updates.
- When Markdown is selected, the editor shows a client-side live preview using
  the same safe renderer as post display. The preview does not call the server
  and raw HTML remains escaped as text. Supported block features include
  headings, pipe tables, lists, block quotes, and fenced code blocks; supported
  inline features include code spans, strong/emphasis, and safe links.
- Creating a root post sets `parent_id = 0`, `root_id = id`, `level = 0`,
  `order_num = 0`, and `reply_count = 1`.
- Creating a root post awards the author points at most once per PostgreSQL
  `CURRENT_DATE` per award category. Normal and announcement posts share the
  regular category and use `ROOT_POST_REGULAR_AWARD_POINTS`, default `2`;
  forward posts use `ROOT_POST_FORWARD_AWARD_POINTS`, default `5`; original
  posts use `ROOT_POST_ORIGINAL_AWARD_POINTS`, default `10`. The award updates
  `user_info.point` only; no `point_log` row is created and `post.point`
  remains unchanged.
- Creating a reply sets `type = 0`, inherits the parent tree and board, sets
  `parent_id` and `level = parent.level + 1`, inserts it at
  `parent.order_num + 1`, shifts later posts in the tree by one position, and
  updates the root post's reply count and latest reply time.
- Creating a root post or reply refreshes denormalized statistics in the same
  transaction: the board's visible `post_count`/`root_count`, the author's
  visible `post_count`/original `doc_count`, and the author's
  `last_post`/`last_origin`/`last_reship` activity timestamps.
- A positive points value on a reply atomically deducts the amount from the
  replying user, credits the owner of the root post being replied to,
  increments that root post's point total, and creates its `point_log` award
  event under the replying user's id so the post page shows who gave the
  points.
- A logged-in user may set a visible post as their signature from the post
  controller only when `post.size <= POST_SIGNATURE_MAX_BYTES`, defaulting to
  1000 bytes. Re-selecting the current signature keeps the existing latest
  signature unchanged; choosing a different eligible post appends a new
  `sign_log` history row.
- Editing changes subject, content, size, visibility, and
  `post.last_update_time`. Editing cannot change content format.
  Administrator root editing can also change type.
  Non-admin root editing preserves the existing type so root-post award
  eligibility cannot be manipulated by later type changes; non-root editing
  keeps `type = 0`. Editing does not change authorship, tree placement,
  existing point awards, signature relationships, existing legacy link
  metadata, or an attached image.
- Images are uploaded as `jpg`, `png`, or `gif` files, validated by media type
  and file signature, copied under a `YYYYMM` subdirectory of
  `IMAGE_DIRECTORY` with a random 128-bit lowercase hex file name, and
  referenced from `post.image_url`. The editor does not accept an image URL.
  The image
  picker displays the configured upload size limit; it defaults to 2 MB.
- An accepted image larger than 500 KB is normalized to a compressed JPEG and
  reduced until its stored size is below 500 KB. Images at or below 500 KB
  retain their uploaded format.
- An attachment may be added during creation/reply publication, but an
  existing attached image cannot be replaced by update mode or the upload
  endpoint.
- Creation and update transactionally refresh the affected board's visible
  post/root counts and the author's visible post/original counts and activity
  timestamps. `last_post` is the latest visible authored post, `last_origin`
  is the latest visible authored original post, and `last_reship` is the latest
  visible authored forward post.
- Successful saves invalidate portal home-cache variants and navigate to the
  saved post page.

### Point Transfer Details

The submitted point field is an optional side effect of creating a reply. It
is not part of root-post creation, post updates, image upload, favorite
changes, signature selection, or deletion. Root-post creation has a separate
automatic author activity award that updates only `user_info.point`.

The editor obtains these values from `GET /api/post_upd?reply_to={post_id}`:

- `reply_points_allowed`: `true` only when the selected reply target is a root
  post owned by another user.
- `current_user_points`: current point balance of the logged-in user.
- `post_reply_max_points`: configured per-reply transfer ceiling.

The frontend shows `Points to author` only when `reply_points_allowed` is true.
Its browser-side maximum is:

```text
min(current_user_points, post_reply_max_points)
```

The hint displays the current user's balance and the configured per-reply
limit. This is only a convenience check; the backend repeats every rule during
`POST /api/post_upd`.

Backend validation accepts a positive point transfer only when all of these are
true:

- The operation is a reply, not root creation and not update.
- The direct reply target is a root post according to the structural root
  helper (`parent_id` empty/zero, `root_id = id`, or `level = 0`).
- The direct reply target belongs to another user.
- The submitted amount is between `1` and `POST_REPLY_MAX_POINTS`.
- The replying user has at least that many points at write time.

Replies that do not meet those conditions may still be created only when their
submitted point value is zero. The frontend hides the point input for self
replies and non-root replies, but the backend rejects invalid positive values
independently of the UI.

On a positive point transfer:

- The replying user's `user_info.point` is decremented.
- The owner of the replied-to root post receives the points through
  `user_info.point`.
- The replied-to root post's aggregate `post.point` is incremented.
- A `point_log` row is inserted with `post_id` equal to the replied-to root
  post and `user_id` equal to the replying user, so post display shows who gave
  the points.

If the submitted points value is `0`, none of the point-balance, post-point, or
`point_log` writes are performed.

## Post Page

Route:

```text
/post/{post_id}
```

Backend data route:

```text
GET /api/posts/{post_id}
```

### Purpose

The post page shows one readable post in full and keeps its surrounding tree
visible for navigation and context.

Browser title:

```text
{post subject} - {site name}
```

### Page Structure

The post page contains:

- Shared header.
- Controller card.
- Full post card.
- Post tree card, using the same compact post-tree component as the board page.
- Shared footer.

The overview intro section is not displayed on this page, leaving the primary
reading flow immediately below the header.

### Controller Card

The controller card contains:

- Board name on the left, linking to `/board/{board_id}`.
- List-view icon linking to `/post_list/{post_id}` for the current tree.
- Print icon opening `/post_print/{post_id}` in a new browser window.
- Favorite heart icon is shown for an authenticated reader on a root post.
  The unselected state sets a favorite; the selected state unsets it. The
  page shows a clear confirmation message after either operation.
- Signature icon is shown for an authenticated reader when the visible post is
  small enough to become a signature
  (`post.size <= POST_SIGNATURE_MAX_BYTES`, default `1000`). Selecting it
  appends a `sign_log` row unless the post is already the user's current
  signature; after success, the same feedback strip used by favorite changes
  reports that the signature was updated.
- Update icon links to `/post_upd?post_id={post_id}` and is shown to the
  owner or an administrator for a root post, but to an administrator only for
  a non-root post. For non-admin users it is hidden when the post has ever
  appeared in `sign_log`.
- Reply icon links logged-in readers to `/post_upd?reply_to={post_id}` only
  while the discussion tree's root post is within
  `POST_REPLY_MAX_AGE_DAYS` of its creation time. In an open tree, anonymous
  readers see a `Login to reply` hint instead of an icon. In an expired tree,
  all viewers see `Replies closed`, because logging in cannot enable a reply.
- Delete icon is shown to an author for an owned root post only while it has
  no children, and to an administrator or board master of the current post's
  board for any post. A populated root therefore requires moderation
  privilege. It opens an inline confirmation; for a root tree with children,
  the confirmation clearly warns that the entire discussion tree becomes
  invisible and states its stored post count.

### Full Post Card

The post card contains:

- Type icon and post title.
- Status pill for related link, image attachment, and encrypted state.
- Metadata with line-drawing icons for author, post time, size, views,
  replies, and non-zero points.
- Post content rendered according to `post.content_format`: plain text keeps
  preserved line breaks and auto-links detected `http`/`https` URLs after
  escaping all other text. Markdown supports a limited client-rendered subset
  with headings, pipe tables, lists, block quotes, code, emphasis, and safe
  links. Raw HTML is escaped and shown as text.
- When a visible post has no body content, `post.has_content = false` renders a
  compact `No content` flag.
- Post body blocks use their natural content height rather than reserving
  extra blank height for short content.
- Optional related link when `post.link_url` is present and uses a safe URL
  scheme, displayed as an accent-colored pill containing a line-drawing link
  icon and link name.
- Optional image attachment.
- Optional signature content referenced by `post.sign_id`. Signature posts use
  the same visibility and content-format rules as normal post content: normal
  signatures are public, while encrypted signatures are visible only to
  authenticated users.
- Optional point-award list sourced from `point_log` when the post has
  non-zero points. The list shows users who gave points to this post, not the
  user who received the points.

Post-author and point-award user names open `/user/{user_id}` in a new browser
window.

### Display Data Rules

- Post metadata is visible for normal and encrypted posts even to anonymous
  readers.
- Post cards show `last_update_time` as `Updated` when a recorded edit time is
  present.
- Full body content is visible when `state = 0`, or when `state = 1` and the
  viewer has a live login session. Anonymous readers of encrypted posts see an
  `Encrypted` pill instead of body content.
- `post.has_content = false` renders a compact `No content` pill, avoiding
  empty body space for posts with no stored content.
- Related links, local image URLs, external image links, signature content,
  and point-award user lists are returned only when the post body is visible.
- `post.point` may appear as metadata for readable post records, but detailed
  point awards are fetched only for visible-content posts with non-zero points.
- Signature content referenced by `post.sign_id` is loaded only if the
  referenced signature post itself is visible under the same normal/encrypted
  rule.
- List view and print view reuse the same body, resource, signature, and point
  visibility rules as the single-post page.
- `GET /api/posts/{post_id}` increments `post.access_count` only for a logged-in
  session, and only once for the same post inside that session. Anonymous
  post-detail reads, list view, print view, board/default/search cards, and
  other metadata-only displays do not increment the view count.

Image behavior:

- A local image path in `post.image_url`, such as
  `202506/c3861443c3f1750b1d5ea5e5e9e10de6.jpg`, is resolved beneath
  `/images` and displayed inline. The stored path is interpreted directly
  relative to `IMAGE_DIRECTORY`; no legacy path prefix is stripped or rewritten.
- For migrated monthly paths, the backend accepts both canonical `YYYYMM/file`
  values and legacy stored `pic/YYYYMM/file` values. Canonical values are read
  directly under `IMAGE_DIRECTORY`. Legacy `pic/` values are stripped first and
  read from the same canonical location, with literal `pic/` path lookup only
  as a last compatibility fallback.
- If a local image reference cannot be loaded by the browser, the broken image
  is replaced with an `Image not found` hint styled like the no-content pill.
- `/images` is backed by the configured `IMAGE_DIRECTORY` filesystem path and
  serves only `jpg`, `jpeg`, `png`, and `gif` attachments. Production should
  point `IMAGE_DIRECTORY` at the unified image root, such as
  `/home/wy/pic/dogn_pic`.
- A local image used only by encrypted posts is served only to a logged-in
  user; an anonymous direct request receives a not-found response.
- A local image referenced only by deleted or unrecognized post states is not
  served, including to logged-in users.
- Local image authorization uses the normalized attachment-path index created
  by `scripts/add_post_image_visibility_index.sql` on an already upgraded
  database.
- An external `http` or `https` image is represented by an accent-colored
  icon-and-label pill link rather than loaded inline.
- Unsafe, traversal-style, or unsupported URLs are not rendered.

### Post Tree Card

The tree card uses the board-page tree rendering component. The current post
item receives a subtle selected background while all visible posts remain
linked to their own detail pages.

### Access And Security

- Normal and encrypted post metadata remains visible to anonymous visitors.
- For an encrypted post (`state = 1`), an anonymous full post card replaces
  its body with `Encrypted`.
- Encrypted body content, link/image locations, inline image access, signature
  content, and detailed point-award listing are available only with a live
  login session.
- Point-award details are also suppressed when the post body is not visible.
  The point total may still appear in metadata, but the `point_log` user list
  is not returned until the viewer can read the post.
- List view and print view apply the same encrypted-content rule.
- Session-dependent API and image responses use `Cache-Control: no-store` so
  authenticated content cannot be redisplayed from browser cache after
  logout.
- Deleted posts (`state = 2`) and posts with unrecognized states are not
  returned.
- Deleting a non-root post or childless root sets only that post to `state =
  2`. Deleting a root with children sets every stored post in that tree to
  `state = 2`. Stored rows and attached resource references are retained;
  existing visibility and image authorization filters then prevent deleted
  posts or their protected resources from being returned.
- A missing or deleted post displays a neutral unavailable state rather than a
  generic data-loading failure.
- Post content, signatures, labels, links, and point user names are escaped
  before HTML rendering.
- API queries bind ids through SQLx parameters.

### Data API

Response shape:

```text
site_name
post
board
tree
boards
```

`boards` is included to populate the shared header board menu. `tree` contains
post summary items in `order_num` display order. `post.content_visible`
indicates whether the body and protected resources may be rendered;
`post.has_link` and `post.has_image` allow attachment indicators to remain
visible when locations are redacted.

`post.point_awards` contains user and point pairs from `point_log` only when
the post content is visible and `post.point` is non-zero. In the post detail
card, these awards display as inline user-name and point-pill pairs on the
same flowing line. `point_log.user_id` identifies the user who gave the
points; the recipient is the owner of the root post being replied to.

`can_delete` and `delete_post_count` describe the permitted deletion operation
and the number of stored posts that its confirmation must warn about.
`can_favorite` and `is_favorite` expose the logged-in viewer's favorite toggle
and current root-post favorite state.

### Operation Logic

- The board label in the controller navigates to `/board/{board_id}` in the
  current window.
- List view navigates to `/post_list/{post_id}` in the current window.
- Delete submits `POST /api/posts/{post_id}/delete` after confirmation. On
  success the browser returns to the post's board because the deleted detail
  page is no longer readable.
- The favorite icon submits `POST /api/posts/{post_id}/favorite` with the
  intended selected state for a logged-in reader on a visible root post. The
  controller reloads to show the resulting icon state and an added/removed
  confirmation. Repeated set requests do not create another relation.
- Print version opens `/post_print/{post_id}` in a new window.
- Eligible owners and administrators can open the post editor for updates.
- The reply icon opens reply mode for authenticated users.
- Safe related-resource and external-image pills open their destinations in
  new windows.
- The compact tree provides navigation to other visible posts in the same
  discussion tree.

### Cache Behavior

The post page is not cached yet. A future write workflow should invalidate
post-detail, board-page, and default-page cache entries affected by a post
change.

### Open Questions

- Whether very large post trees should use truncation or lazy expansion in the
  context card rather than rendering the full tree at once.

## Post List Page

Route:

```text
/post_list/{post_id}
```

Backend data route:

```text
GET /api/post_lists/{post_id}
```

### Purpose

The post list page reads an entire post tree as full post cards rather than
showing one full card followed by compact tree navigation. It is opened from
the list-view action on a post page.

Browser title:

```text
{selected post subject} - {site name}
```

### Page Structure

The post list page contains:

- Shared header and footer.
- A controller card containing only the board link; post actions are omitted in
  this aggregate reading view.
- One full-width post card for each visible post in the selected post's tree,
  reusing the single-post card presentation.
- A compact post-tree navigation card after the full post cards.

Full post cards are ordered oldest first by creation time
(`post.post_time ASC`, with ascending id as a stable tie-breaker). Card
content, resources, signature rendering, encrypted-resource redaction, and
point-award visibility use the same presentation and escaping rules as the
single-post page. Selecting the subject in a full post card opens that post's
single-post page. The compact trailing tree continues to follow maintained
`order_num` discussion order.

When the selected post is a reply rather than the root, the page scrolls to
its full card after loading and briefly pulses its selected background. The
animation is disabled when the browser requests reduced motion.

### Operation Logic

- The controller contains the board navigation link only; list/print/reply
  tools are intentionally omitted in this aggregate reading view.
- Selecting a full-card subject opens `/post/{post_id}` in the current window.
- The compact trailing tree retains discussion-context navigation using the
  shared tree component and `order_num` sequence, independent of the
  oldest-first full-card order.
- The page renders the complete visible tree and does not paginate it.

### Data API

`/api/post_lists/{post_id}` returns:

```text
site_name
selected_post_id
board
posts
boards
```

The backend resolves the tree from the requested post, loads visible full
posts ordered by `post_time ASC, id ASC`, includes `order_num` for compact
tree presentation, joins signature content visible to the current viewer, and
fetches point awards in one batched lookup for visible-content posts with
non-zero points.

## Post Print Page

Route:

```text
/post_print/{post_id}
```

Backend data route:

```text
GET /api/post_prints/{post_id}
```

### Purpose

The print page provides a clean formatted representation of one post in a new
browser window, suitable for browser printing.

### Page Structure

The page contains only printable post content:

- Subject as the document heading.
- A metadata line beginning with the small site logo and configured site name,
  followed by plain-text post metadata.
- Post body.
- Optional textual related link.
- Optional local image or textual external-image link.
- Optional signature and point awards.

It intentionally excludes the shared header, footer, controller, post type and
status icons, and surrounding post-tree navigation. Dynamic post values use
the same escaping and URL validation rules as the interactive post page. Its
API returns only printable post data and board context, without fetching the
surrounding tree or header board-navigation data.

### Operation Logic

- The page is opened by the post-page print tool in a new window.
- The application supplies no toolbar or navigation actions; printing is
  performed through the browser.
- Safe related URLs and external-image URLs remain simple clickable text
  links.
- Encrypted post visibility is evaluated before the printable content is
  returned. Printable signatures and point awards follow the same visibility
  rules as the interactive post page.

## User Administration Workflow

The implemented user-management flow is split across three pages:

| Page | Primary audience | Implemented purpose |
| --- | --- | --- |
| `/user_add` | Administrator | Create a member account with an initial modern password and optional introducer. |
| `/user_list` | Administrator | Search, filter, and inspect accounts before opening a profile. |
| `/user/{user_id}` | Any reader; owner/admin for writes | Read public activity and perform authorized account/profile operations. |

The header account menu exposes `Add user` and `User list` only for an
administrator. These are navigation conveniences; every API operation
independently rechecks the current stored administrator level. User creation,
directory access, profile update, password change, statistics recalculation,
and role assignment are implemented. No browser control grants a permission
that is not also enforced by the API.

Account creation and later update deliberately use separate operations:

- `POST /api/users` creates a new member with `name`, optional `email`,
  optional `intro`, optional `intro_user_id`, and an initial password.
- `POST /api/users/{user_id}/profile` updates only `email` and `intro`; it
  does not change identity, password, role, statistics, or introducer.
- `POST /api/users/{user_id}/password`,
  `POST /api/users/{user_id}/statistics/recalculate`, and
  `POST /api/users/{user_id}/role` provide the corresponding explicit
  operations, with their own permission rules.

This separation keeps privileged changes visible and avoids treating general
profile editing as a path to role or authentication changes.

## User Page

Route:

```text
/user/{user_id}
```

Backend data route:

```text
GET /api/users/{user_id}?activity=original&page=1&page_size=50
```

### Purpose

The user page presents public profile context and the user's post-related
activity. Interactive user-name links, including post cards, portal user
lists, point-award lists, and introducing-user metadata, open this page in a
new browser window. The authenticated header menu's `Profile` command opens
the logged-in user's own page in the current window.

Browser title:

```text
{user name} - {site name}
```

### Page Structure

The page uses the shared header and footer and contains:

- A full-width status card with the user icon, role derived from level,
  registration date, last login time, introduction, and latest readable
  signature. Post count, original-post count (`doc_count`), and current point
  total use the established metric pill style. User metadata icons are
  followed by visible field labels so their meaning does not rely on icon
  interpretation or hover text.
  Introduction and latest readable signature text appear together in the
  profile body with compact visible section labels. The signature section is
  labeled `Signature` and uses the same italic tone as signatures rendered
  under post content.
- Operation icon controls visible only when the viewer owns the profile or has
  administrator level (`level >= 10`). An update icon after recalculation
  edits email and introduction. A set-role icon after update is shown only to
  administrators.
- An activities panel with tabs for original posts, favorite posts, and
  signature-history posts.
- A pager below the activity list using the selected activity tab and page.

The change-password operation opens an inline form. Owners supply their
current password; administrators may replace any user's password without the
existing credential. The recalculate-statistics icon opens an inline
confirmation panel that states which counts will be rebuilt and which post
states are excluded. Confirming performs the update for an authorized target.
The update icon opens an inline form for the profile email and introduction;
owners and administrators may perform this update.
The set-role icon opens a role form offering frozen, member, and
administrator; Advanced is managed by board-master membership rather than
selected manually.

### Activity Data

Activity tabs select these datasets:

- `original`: posts authored by this user whose post type is original.
- `favorites`: posts related through `favorite.user_id` and
  `favorite.post_id`.
- `signatures`: posts related through historical `sign_log.user_id` and
  `sign_log.sign_id` records.

All datasets include only viewable post states (`normal` and `encrypted`),
order posts by `post.id DESC`, and are paged with a default page size of 50
and an API limit of 100 per page. An empty tab shows a simple empty state.

### Authorization And Privacy

Anonymous viewers may see the profile and post-card metadata, including cards
for encrypted posts, in line with other post listings. Links and image
resource URLs belonging to encrypted posts are withheld until a valid session
is present. The endpoint sends `Cache-Control: no-store` because its response
depends on authentication state and exposes update authorization.

The API reports `can_update` only for the profile owner or an administrator.
Any future mutation endpoint must independently enforce the same rule; hiding
operation controls in the browser is not an authorization control.

The email address, last login IP address, introducing user's name, and login
counter are confidential profile details. The backend includes
`private_details` only when `can_update` is true, so they do not reach
unauthorized browser clients. When available, they use the same labeled
metadata style and line as the public profile metadata. The introducing user's
name links to that user's profile in a new browser window.

The latest signature is selected using the newest `sign_log` record. Its
content is included only when that selected signature post is publicly
readable; the page does not substitute an older signature when the latest one
cannot be displayed.

### Data API

Response shape:

```text
site_name
user
latest_signature
private_details (owner/admin only; omitted otherwise)
can_update
can_set_role
activity
pager
posts
boards
```

`user` contains public status values. `posts` contains the selected paged
activity list, using metadata visibility rules equivalent to the portal post
cards. `boards` populates the shared portal/board menu without a second data
request.

### Operation Logic

- Selecting an activity tab resets paging to page `1` and loads that dataset.
- Activity pager controls preserve the selected tab and update only its page
  query value.
- Activity post items reuse portal post-card rendering and open post pages in
  new windows.
- The authenticated user's `Profile` account-menu command opens their own
  profile in the current window.
- Only the profile owner or administrator (`level >= 10`) receives
  `private_details` and sees profile-operation controls.
- Only administrators receive `can_set_role = true` and see the set-role
  control after the update icon.
- Update profile submits to `POST /api/users/{user_id}/profile`; an owner can
  update their own email and introduction, and an administrator can update
  either field for any account. Email remains private while introduction is
  public profile content. Empty submitted values are stored as absent values;
  email is capped at 25 characters and introduction at 100 characters to
  match existing column capacity.
- Change password submits to `POST /api/users/{user_id}/password`; owner
  changes require the current password, administrator resets do not, and a
  successful change invalidates sessions belonging to the target account. The
  form shows a generated password suggestion by default with regenerate and
  copy controls; it is not inserted into input fields automatically.
- Recalculate statistics submits to
  `POST /api/users/{user_id}/statistics/recalculate`; owners can recalculate
  their own statistics and administrators can recalculate any account.
- Recalculation writes `post_count` as the number of authored normal or
  encrypted posts, `doc_count` as the number of authored original posts in
  those states, and `favorite_count` as the number of favorites whose target
  post is in those states. It also refreshes `last_post`, `last_origin`, and
  `last_reship` from the latest visible authored post, original post, and
  forward post respectively. Deleted and unknown-state posts are excluded.
- Successful recalculation invalidates portal home-cache variants and reloads
  the current user view from its JSON API so displayed values come from the
  updated database state. The request uses the same custom-header CSRF check
  as password change.
- Set role submits to `POST /api/users/{user_id}/role`; only administrators
  can request Frozen (`0`), Member (`1`), or Administrator (`10`). Requesting
  Member for a user who is a board master resolves to Advanced (`5`) while
  that relationship remains. A changed role invalidates that user's active
  sessions.

## User List Page

Route:

```text
/user_list
```

Backend data route:

```text
GET /api/users?query=&role=&order=id_desc&page=1&page_size=50
```

### Purpose And Access

The user list is an administrator-only directory for inspecting member
accounts. The shared HTML shell may load before the browser requests dynamic
data, but the JSON API returns directory rows only when the request carries an
authenticated administrator session (`level >= 10`). Anonymous and
non-administrator sessions receive an access-denied state and never receive
email addresses or directory records. Session authorization resolves the
current stored user level, so an already-issued administrator session does not
retain directory access after a downgrade or freeze.

### Page Structure

The page uses the shared header and footer and contains:

- A full-width filter card with one text search field, a role selector, an
  id-order selector, and a search command.
- A full-width, horizontally scrollable table for the directory results.
- A pager below the table.

Each result row includes the user's id, name, role, email, registration time,
post count, original-post count, favorite count, points, and last login time.
User names link to `/user/{user_id}` in a new browser window, consistent with
other profile links.

### Query And Paging Logic

- `query` performs a case-insensitive substring match against trimmed user
  names and email addresses.
- `role` optionally restricts rows to active accounts (`active`, any
  non-frozen level) or one known user level: frozen (`0`), member (`1`),
  advanced (`5`), or administrator (`10`).
- `order=id_desc` is the default and lists newer ids first.
- `order=id_asc` lists older ids first.
- `page_size` defaults to 50 and is capped at 100 by the API.
- Applying a new search or order value returns to the first page.
- An empty result set shows a single clear empty-table state.

### Response And Security

The response contains `site_name`, normalized search/order values, `role`,
`active`, a user pager, `users`, and `boards` for the shared header menu.
`active = true` identifies the combined non-frozen filter while `role`
continues to identify a selected numeric role. The endpoint uses
`Cache-Control: no-store` because it exposes privileged account data. Menu
visibility is only a convenience; the API performs the authorization check.

## User Add Page

Route:

```text
/user_add
```

Backend mutation route:

```text
POST /api/users
```

Request fields:

```text
name
email (optional)
intro (optional)
intro_user_id (optional)
password
confirm_password
```

### Purpose And Access

The user-add page is an administrator-only account creation form. It is
available from the authenticated account menu only to administrators
(`level >= 10`). The JSON mutation endpoint independently resolves the live
session user and rejects anonymous or non-administrator requests.

### Page Structure

The page uses the shared header and footer and shows one full-width form card
containing:

- User name, limited to the legacy database capacity of 25 characters.
- Optional email, limited to the legacy database capacity of 25 characters.
- Optional introduction, limited to the legacy database capacity of 100
  characters.
- Optional introducing user selected by searching the existing user list by
  name or email.
- A cryptographically random valid password suggestion with controls to
  generate another suggestion or copy the displayed value.
- Password and confirmation fields with the established password-policy
  guidance.

After a successful create response, the page navigates to the new user's
profile so the administrator can inspect the resulting account.

### Operation Logic And Security

- The form submits JSON with the custom same-origin mutation header.
- The backend always creates a new account as a member (`level = 1`); role
  assignment is not part of account creation.
- Generated password suggestions are browser-only convenience values; the
  administrator must place the selected value into the password fields and
  the backend applies the password policy to submitted data.
- The backend validates name/email/introduction capacity, an optional existing
  introducing-user id, password confirmation, and the shared new-password
  policy.
- A new credential is stored directly as `argon2id-v1`; no MD5-derived
  password representation is created for a new account.
- Trimmed user names already present in `user_info` are rejected, because
  login uses the trimmed name as its lookup key.
- Account creation initializes counters to zero, records registration time,
  sets the initial point balance from `NEW_USER_INITIAL_POINTS` (default
  `100`), stores optional `intro` and `intro_user_id` values, and invalidates
  portal home-cache variants so new-user summaries update.
- The create response returns the new `user_id`; the browser then opens that
  profile, where an administrator can update the profile, reset its password,
  recalculate statistics, or explicitly set an allowed role.

## Site Manager Page

Route:

```text
/site_mgr
```

Backend routes:

```text
GET  /api/site_manager
POST /api/site_manager/categories
POST /api/site_manager/categories/{category_id}
POST /api/site_manager/categories/{category_id}/delete
POST /api/site_manager/boards
POST /api/site_manager/boards/{board_id}
POST /api/site_manager/boards/{board_id}/delete
POST /api/site_manager/boards/{board_id}/masters
POST /api/site_manager/boards/{board_id}/masters/{user_id}/remove
POST /api/site_manager/boards/statistics/recalculate
```

### Purpose And Access

The site manager is an administrator-only operations page for the existing
forum taxonomy. The API resolves current session authorization and returns no
management data to anonymous, member, or downgraded administrator sessions.
All mutations require the same-origin request header used by other protected
updates.

### Page Structure

The page uses the shared header and footer. It contains full-width category
sections ordered by category order and id. Each category section includes:

- Editable category name, description, and display order fields.
- A board-count badge derived from current board membership.
- Commands to add a board and to delete the category.
- Board management rows for boards assigned to that category.

A controller section appears before the category sections and contains one
command to add a category and one confirmed command to recalculate statistics
for all boards.

Each board row includes:

- Link to the readable board page.
- Current post and root count pills.
- Editable board name, description, category placement, display order, and
  assigned board masters shown by name with remove commands.
- A delete-board command.
- An add-master command that opens an inline search form. Administrators search
  for a user by name and select a matching user instead of loading the full
  user directory into a selector.

Successful category and board-metadata edits reload the management data and
shared board navigation so names, ordering, and category assignments
immediately reflect stored values. Board-master add/remove commands persist
immediately and update only the assignment display, preserving unsaved board
metadata input values in the current form.

### Operation Logic

- Category updates modify `name`, `comment`, and `order_id`.
- Creating a category inserts a new empty category with generated ID; creating
  a board inserts an empty board under the selected category with generated ID.
- A category can be deleted only when no `board` row references it. A board can
  be deleted only when no `post` row references it. These checks use live
  relationship rows, not stored count values.
- Board updates modify `name`, `comment`, `category_id`, and `order_id`.
  Creating, moving, or deleting a board refreshes stored
  `category.board_count` values in the same transaction.
- Adding a selected master immediately inserts a `board_master` relationship;
  a duplicate assignment is rejected. If the selected user is currently a
  Member, this operation also changes the user to Advanced.
- Removing a shown master immediately deletes its `board_master` relationship
  and compacts remaining `order_id` values to preserve stable display order.
  If an Advanced user no longer manages any board after removal, the operation
  changes that user back to Member.
- Deleting an empty board cascades its board-master assignments and applies the
  same role reconciliation before the transaction is committed.
  Both operations require administrator authorization and same-origin
  mutation verification. Arbitrary free-text names are not stored.
- Automatic board-master level transitions apply only between Member and
  Advanced. Administrator and Frozen roles are not altered by adding or
  removing board-master relationships.
- The manager response contains only existing assignments. Candidate users are
  requested on demand while the add-master search is open, so the page payload
  does not grow with the full user population.
- The controller's statistics recalculation updates every board's
  `post_count` and `root_count` from posts in states `normal` and
  `encrypted`; deleted and unsupported-state posts are excluded. A root post
  has no effective parent
  (`COALESCE(parent_id, 0) = 0`). It also refreshes every stored category
  board count and repairs board-master derived roles: Members with an
  assignment become Advanced and Advanced users with no assignments become
  Members. Frozen users and administrators are unchanged.
- Every successful board or category mutation invalidates portal home-cache
  variants because portal and header navigation expose board/category data.

Successful category or board creation/deletion invalidates portal and
navigation cache variants in the same way as metadata edits.

## Future Page Sections

Future page designs should be added to this document with:

- Route.
- Purpose.
- Page structure.
- Required API endpoints.
- Data shape.
- Loading, empty, and error states.
- Cache behavior.
- Accessibility notes.
- Security notes.
- Open questions.
