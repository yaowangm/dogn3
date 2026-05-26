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

## Page Inventory

| Page | Browser route | JSON API used | Purpose | Primary operations |
| --- | --- | --- | --- | --- |
| Portal / default | `/` | `GET /api/home` | Overview of recent posts, users, and boards. | Open posts or users in new windows; enter boards; open header menus or login. |
| Login | `/login` | `GET /api/auth/session`, `POST /api/auth/login`, `POST /api/auth/logout` | Establish or end an authenticated session. | Submit credentials; return to the originating local page after login/logout. |
| Board | `/board/{board_id}` | `GET /api/boards/{board_id}?page={page}` | Read paged post trees in one board. | Page through results; open posts or authors in new windows. |
| Post | `/post/{post_id}` | `GET /api/posts/{post_id}` | Read one full post with tree context. | Enter board; switch to list view; open print view; open user/resource links. |
| Post list | `/post_list/{post_id}` | `GET /api/post_lists/{post_id}` | Read a complete discussion tree as full post cards. | Navigate full posts and compact tree; focus the selected reply. |
| Post print | `/post_print/{post_id}` | `GET /api/post_prints/{post_id}` | Minimal browser-printable post representation. | Use browser print; follow safe related links. |
| User | `/user/{user_id}` | `GET /api/users/{user_id}?activity={activity}&page={page}`, profile/password/statistics mutation endpoints, and `POST /api/users/{user_id}/role` | View profile status and activity. | Select activity/page; open posts/users; update permitted profile fields; change password; recalculate statistics; administrators set roles. |
| User list | `/user_list` | `GET /api/users?query={query}&role={role}&order={order}&page={page}` | Administrator-only member directory. | Search names/email; filter roles; sort by id; page results; open profiles. |
| User add | `/user_add` | `GET /api/users?query={query}` and `POST /api/users` | Administrator-only account creation. | Enter identity and initial password; optionally select an introducer; open the created profile. |
| Site manager | `/site_mgr` | `GET /api/site_manager`, category/board mutation endpoints, and `POST /api/site_manager/boards/statistics/recalculate` | Administrator-only board/category maintenance. | Create/edit/delete empty taxonomy nodes, manage board masters, and recalculate all board statistics. |

Supporting routes that are not browser pages:

| Route | Purpose | Authorization-sensitive behavior |
| --- | --- | --- |
| `GET /api/health` | Report service/database/cache health. | Not used for user navigation. |
| `GET /images/{*path}` | Serve approved local post image attachments. | Encrypted-only images require login; deleted/unknown-only images fail closed. |

## Routing And Interaction Model

All interactive pages except print use the shared HTML shell and the
`dogn-app-shell` native Web Component. The component recognizes the browser
path, requests its JSON endpoint, then renders dynamic content. The print page
uses a minimal shell without the shared header and footer.

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
| Safe external resource pill | Valid `http`/`https` URL | New window | Uses `noopener noreferrer`; unsafe targets are omitted. |

Current mutation boundary:

- Login, logout, authorized password change, and authorized user-statistics
  recalculation are implemented state-changing UI operations.
- Reply, profile editing, favorite changes, and moderation workflows are not
  implemented.
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

### Dynamic Data And Failure States

- Browser page HTML is a shell; dynamic forum content is loaded from JSON
  endpoints through Ajax.
- Each interactive data page begins with loading text or loading sections.
- Endpoint failure leaves the shell visible and replaces page content with a
  neutral failure state.
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
- `Search` is a reserved destination for a future page.
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

- Implement and refine the reserved search destination.
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
- Real route and workflow for search.
- Whether `/api/home` should use more granular cache keys later.
- Whether the default page should include pagination or only fixed overview
  lists.

## Login Page

Route:

```text
/login
```

Backend API routes:

```text
POST /api/auth/login
GET  /api/auth/session
POST /api/auth/logout
```

### Purpose

The login page authenticates an existing forum user using the migrated
credential representation and establishes an opaque server-managed session.

### Page Structure

The login page contains:

- Shared header and footer.
- A focused login card.
- Labeled user-name input.
- Labeled password input.
- Login submit button.
- Generic invalid-credentials error state.

The login form submits JSON through Ajax. Credentials are never placed in a
URL. The login link carries only a local `return_to` page destination. On
success the browser receives an `HttpOnly` session cookie and returns to the
page that opened login, falling back to the portal page when no valid previous
page is available. Once the session is detected, the shared header displays a
user icon-and-name menu trigger rather than the login link. Logout uses a POST
API action and reloads the current page in anonymous state, with the same
portal fallback if no valid local page is available.

### Operation Logic

- Page initialization calls `GET /api/auth/session` to render the login link
  or authenticated account menu.
- Form submission calls `POST /api/auth/login` with JSON `name` and
  `password`; credentials are never placed in a URL.
- Successful login issues the opaque session cookie and navigates to a valid
  local `return_to` route or `/`.
- A failed login renders the same failure message for invalid credentials,
  unknown users, frozen accounts, and unsupported/unmigrated credentials.
- Header logout calls `POST /api/auth/logout`, clears the live session cookie,
  and reloads the prior local page as an anonymous visitor.

### Security Notes

- Invalid, unknown, frozen, and unmigrated accounts receive the same visible
  login failure message.
- Frozen accounts are identified by `user_info.level = 0`;
  `user_info.state` does not control login eligibility.
- Password inputs are processed only by the authentication API.
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
- Pager controller.
- Direct post tree cards.
- Pager controller.
- Shared footer.

The top and bottom pager controllers contain the same content so users can move
between pages before or after reading the current list.

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
- No board write, subscription, moderation, or post-creation action is
  implemented on this page.

### Cache Behavior

The board page is not cached yet.

Future cache keys should include board id and page number, for example:

```text
api:board:{board_id}:page:{page}:v1
```

Any post write inside a board should invalidate affected board-page keys and
the default page cache key.

### Open Questions

- Final page size and whether users can choose it.
- Whether request-provided `page_size` should remain public or become internal
  only.
- Whether board page data should be cached before write flows exist.
- Whether board masters should eventually link to user profile pages.

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
- Reply icon reserved for the future reply workflow.

### Full Post Card

The post card contains:

- Type icon and post title.
- Status pill for related link, image attachment, and encrypted state.
- Metadata with line-drawing icons for author, post time, size, views,
  replies, and non-zero points.
- Plain-text post content rendered with preserved line breaks.
- When a visible post has no body content, `post.has_content = false` renders a
  compact `No content` flag.
- Post body blocks use their natural content height rather than reserving
  extra blank height for short content.
- Optional related link when `post.link_url` is present and uses a safe URL
  scheme, displayed as an accent-colored pill containing a line-drawing link
  icon and link name.
- Optional image attachment.
- Optional signature content referenced by `post.sign_id`.
- Optional point-award list sourced from `point_log` when the post has
  non-zero points.

Post-author and point-award user names open `/user/{user_id}` in a new browser
window.

Image behavior:

- A local image path in `post.image_url`, such as `pic/200809/example.JPG`,
  is resolved beneath `/images` and displayed inline.
- `/images` is backed by the configured `IMAGE_DIRECTORY` filesystem path and
  serves only `jpg`, `jpeg`, `png`, and `gif` attachments.
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
- List view and print view apply the same encrypted-content rule.
- Session-dependent API and image responses use `Cache-Control: no-store` so
  authenticated content cannot be redisplayed from browser cache after
  logout.
- Deleted posts (`state = 2`) and posts with unrecognized states are not
  returned.
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
post summary items in `order_num` display order. `post.point_awards` includes
user and point pairs from `point_log`. In the post detail card, these awards
display as inline user-name and point-pill pairs on the same flowing line.
`post.content_visible` indicates whether the body and protected resources may
be rendered; `post.has_link` and `post.has_image` allow attachment indicators
to remain visible when locations are redacted.

### Operation Logic

- The board label in the controller navigates to `/board/{board_id}` in the
  current window.
- List view navigates to `/post_list/{post_id}` in the current window.
- Print version opens `/post_print/{post_id}` in a new window.
- The reply icon is reserved; no reply write workflow exists.
- Safe related-resource and external-image pills open their destinations in
  new windows.
- The compact tree provides navigation to other visible posts in the same
  discussion tree.

### Cache Behavior

The post page is not cached yet. A future write workflow should invalidate
post-detail, board-page, and default-page cache entries affected by a post
change.

### Open Questions

- Reply editor workflow and mutation API.
- Whether post views should increment `access_count`.
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
content, resources, signature rendering, and point awards use the same
presentation and escaping rules as the single-post page. Selecting the subject
in a full post card opens that post's single-post page. The compact trailing
tree continues to follow maintained `order_num` discussion order.

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
tree presentation, joins visible signature content, and fetches point awards
in one batched lookup for posts with non-zero points.

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
  returned.

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
  Introduction and latest signature text use compact visible section labels.
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
  successful change invalidates sessions belonging to the target account.
- Recalculate statistics submits to
  `POST /api/users/{user_id}/statistics/recalculate`; owners can recalculate
  their own statistics and administrators can recalculate any account.
- Recalculation writes `post_count` as the number of authored normal or
  encrypted posts, `doc_count` as the number of authored original posts in
  those states, and `favorite_count` as the number of favorites whose target
  post is in those states. Deleted and unknown-state posts are excluded.
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
  stores optional `intro` and `intro_user_id` values, and invalidates portal
  home-cache variants so new-user summaries update.
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
- Board updates modify `name`, `comment`, `category_id`, and `order_id` only.
- Adding a selected master immediately inserts a `board_master` relationship;
  a duplicate assignment is rejected. If the selected user is currently a
  Member, this operation also changes the user to Advanced.
- Removing a shown master immediately deletes its `board_master` relationship
  and compacts remaining `order_id` values to preserve stable display order.
  If an Advanced user no longer manages any board after removal, the operation
  changes that user back to Member.
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
  (`COALESCE(parent_id, 0) = 0`).
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
