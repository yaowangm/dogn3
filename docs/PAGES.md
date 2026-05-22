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
- Future logged-in state should replace `login` with a user icon button and a
  user menu.

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
- Reply count.
- View count.
- Point count.

The entire item is visually treated as clickable through the title link overlay.

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
- Displays image attachment icon when `post.image_url` is present.
- Displays encrypted icon when `post.state = 1`.
- Uses a blank background and black icon/border treatment.

Deleted posts are excluded by the backend query.

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
- Deleted posts are excluded with `state <> 2`.
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
api:home:v1
```

Behavior:

- If cache is enabled and contains a valid response, return cached JSON.
- On cache miss, read PostgreSQL and write the response to Redis.
- Redis read/write runtime errors are logged and fall back to PostgreSQL.
- If cache is disabled, PostgreSQL is always used.

Current invalidation:

- TTL-only via `REDIS_DEFAULT_TTL_SECONDS`.
- No database-write-driven invalidation exists yet.

Future invalidation:

- Post, user, board, and category writes should invalidate `api:home:v1` after
  successful database transactions.

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

- Confirm menu semantics once real navigation and logged-in user state are
  implemented.
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
- Real authentication/session model and logged-in header state.
- Real routes for posts, boards, users, profile, search, login, and logout.
- Whether `/api/home` should use more granular cache keys later.
- Whether the default page should include pagination or only fixed overview
  lists.

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

- Board name.
- Board comment.
- Category name.
- Post count.
- Root/thread count.
- Board master names from `board.master_name`, `board.master_name_2`,
  `board.master_name_3`, and `board.master_name_4`.

Empty board master fields are ignored. If no master names are present, the UI
shows a neutral fallback.

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
traversal order.

Each post item includes:

- Post type icon.
- Post title.
- Author name or fallback user id.
- Post time.
- Access/view count.
- Point count.
- Status bar.
- Latest reply time for root posts only.

The status bar is aligned to the right side of the post title row.

Each post item is indented according to `post.level`. The root has level `0`;
direct replies have level `1`; deeper replies continue to indent. The UI caps
extreme indentation so very deep trees remain readable.

### Data API

Endpoint:

```text
GET /api/boards/{board_id}?page=1&page_size=10
```

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

Visible post trees are fetched in one SQL query:

- Select root posts for the requested board and page.
- Join all posts whose `root_id` belongs to those roots.
- Exclude deleted posts with `state <> 2`.
- Order trees by newest root first.
- Order posts inside each tree by `order_num`.

This follows the database design rule that many post trees can be displayed in
correct depth-first order by sorting with root order and `order_num`.

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
- Whether board page data should be cached before write flows exist.
- Exact post detail route and access control behavior for encrypted posts.
- Whether board masters should eventually link to user profile pages.

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
