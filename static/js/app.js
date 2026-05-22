const defaultHeaders = {
  "Accept": "application/json",
};

async function getJson(path, options = {}) {
  const response = await fetch(path, {
    ...options,
    headers: {
      ...defaultHeaders,
      ...options.headers,
    },
  });

  if (!response.ok) {
    throw new Error(`Request failed: ${response.status}`);
  }

  return response.json();
}

function getHome() {
  return getJson("/api/home");
}

function getBoard(boardId, page = 1) {
  return getJson(`/api/boards/${encodeURIComponent(boardId)}?page=${encodeURIComponent(page)}`);
}

const brandIcon = `
  <svg class="brand__logo" aria-hidden="true" viewBox="0 0 100 100" width="40" height="40" xmlns="http://www.w3.org/2000/svg">
    <g transform="rotate(90, 50, 50)">
      <path d="M25 66.67 L50 16.67" fill="none" stroke="black" stroke-width="14" stroke-linecap="round" />
      <path d="M25 66.67 L50 16.67" fill="none" stroke="white" stroke-width="6" stroke-linecap="round" />
      <path d="M50 16.67 L75 66.67" fill="none" stroke="black" stroke-width="14" stroke-linecap="round" />
      <path d="M50 16.67 L75 66.67" fill="none" stroke="white" stroke-width="6" stroke-linecap="round" />
      <path d="M75 55 L25 55" fill="none" stroke="black" stroke-width="14" stroke-linecap="round" />
      <path d="M75 55 L25 55" fill="none" stroke="white" stroke-width="6" stroke-linecap="round" />
    </g>
  </svg>
`;

const userIcon = `
  <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
    <circle cx="12" cy="8" r="3.5" />
    <path d="M5.5 19c1.2-3.2 3.4-4.8 6.5-4.8s5.3 1.6 6.5 4.8" />
  </svg>
`;

const postTypeLabels = {
  0: "Normal",
  1: "Original",
  2: "Forward",
  3: "Announce",
};

const postTypeClasses = {
  0: "post-type-normal",
  1: "post-type-original",
  2: "post-type-forward",
  3: "post-type-announce",
};

const defaultSiteName = "Dogn";

const sectionIcons = {
  posts: `
    <svg class="section__icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M5 5.5h14" />
      <path d="M5 10h11" />
      <path d="M5 14.5h14" />
      <path d="M5 19h8" />
    </svg>
  `,
  users: `
    <svg class="section__icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <circle cx="9" cy="8" r="3" />
      <path d="M3.5 19c.9-3.4 2.7-5 5.5-5s4.6 1.6 5.5 5" />
      <path d="M16 7.5a2.6 2.6 0 0 1 0 5" />
      <path d="M17.5 14.5c1.8.7 2.9 2.2 3.3 4.5" />
    </svg>
  `,
  boards: `
    <svg class="section__icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <rect x="4" y="4" width="7" height="7" rx="1.5" />
      <rect x="13" y="4" width="7" height="7" rx="1.5" />
      <rect x="4" y="13" width="7" height="7" rx="1.5" />
      <rect x="13" y="13" width="7" height="7" rx="1.5" />
    </svg>
  `,
};

const postTypeIcons = {
  0: `
    <svg class="type-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M6 6h12" />
      <path d="M6 11h12" />
      <path d="M6 16h7" />
    </svg>
  `,
  1: `
    <svg class="type-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M12 4l2.3 4.7 5.2.8-3.8 3.7.9 5.2-4.6-2.5-4.6 2.5.9-5.2-3.8-3.7 5.2-.8L12 4z" />
    </svg>
  `,
  2: `
    <svg class="type-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M7 7h10l-3-3" />
      <path d="M17 7l-3 3" />
      <path d="M17 17H7l3 3" />
      <path d="M7 17l3-3" />
    </svg>
  `,
  3: `
    <svg class="type-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M5 13V8.5L15 5v14L5 15.5V13z" />
      <path d="M5 13H3.5" />
      <path d="M8 16l1.5 4" />
      <path d="M19 9.5v5" />
    </svg>
  `,
};

const userListIcon = `
  <svg class="item-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
    <circle cx="12" cy="8" r="3.2" />
    <path d="M5.5 19c1.2-3.3 3.4-5 6.5-5s5.3 1.7 6.5 5" />
  </svg>
`;

const boardListIcon = `
  <svg class="item-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
    <rect x="5" y="5" width="14" height="14" rx="2" />
    <path d="M9 9h6" />
    <path d="M9 13h6" />
    <path d="M9 17h3" />
  </svg>
`;

const attachmentIcons = {
  image: `
    <svg class="status-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <rect x="4" y="5" width="16" height="14" rx="2" />
      <circle cx="9" cy="10" r="1.6" />
      <path d="M7 17l4.5-4.5 2.8 2.8 1.5-1.5L20 18" />
    </svg>
  `,
  encrypted: `
    <svg class="status-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <rect x="5" y="10" width="14" height="10" rx="2" />
      <path d="M8 10V7.5a4 4 0 0 1 8 0V10" />
      <path d="M12 14v2" />
    </svg>
  `,
};

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function postTitle(post) {
  return escapeHtml(post.subject || "(untitled)");
}

function meta(parts) {
  return parts.filter(Boolean).map(escapeHtml).join(" · ");
}

function renderPostStatusBar(post) {
  const icons = [];

  if (post.image_url) {
    icons.push(`<span title="Has image attachment">${attachmentIcons.image}</span>`);
  }

  if (Number(post.state) === 1) {
    icons.push(`<span title="Encrypted post">${attachmentIcons.encrypted}</span>`);
  }

  if (!icons.length) {
    return "";
  }

  return `<span class="status-bar" aria-label="Post status">${icons.join("")}</span>`;
}

function siteNameFrom(data) {
  const siteName = data?.site_name?.trim();
  return siteName || defaultSiteName;
}

function groupedBoards(boards) {
  const groups = [];
  const groupIndex = new Map();

  boards.forEach((board) => {
    const categoryId = board.category_id ?? "uncategorized";
    if (!groupIndex.has(categoryId)) {
      groupIndex.set(categoryId, groups.length);
      groups.push({
        id: categoryId,
        name: board.category_name || "Other",
        boards: [],
      });
    }

    groups[groupIndex.get(categoryId)].boards.push(board);
  });

  return groups;
}

class DognAppShell extends HTMLElement {
  constructor() {
    super();
    this.session = { loggedIn: false, user: null };
  }

  connectedCallback() {
    this.render();
    this.loadCurrentPage();
  }

  loadCurrentPage() {
    const boardId = this.currentBoardId();
    if (boardId) {
      this.loadBoard(boardId, this.currentPage());
      return;
    }

    this.loadHome();
  }

  currentBoardId() {
    const match = window.location.pathname.match(/^\/board\/(\d+)\/?$/);
    return match?.[1] || null;
  }

  currentPage() {
    const page = Number(new URLSearchParams(window.location.search).get("page") || "1");
    return Number.isInteger(page) && page > 0 ? page : 1;
  }

  render() {
    this.innerHTML = `
      <div class="app-shell">
        ${this.renderHeader()}
        <div class="page-mask" hidden data-page-mask aria-hidden="true"></div>
        <main class="main" id="main-content">
          <section class="intro" aria-labelledby="page-title">
            <p class="eyebrow">Forum</p>
            <h1 id="page-title">${escapeHtml(defaultSiteName)}</h1>
            <p>Recent discussions, original posts, forwards, users, and boards.</p>
          </section>
          <section class="dashboard" aria-label="Forum overview">
            ${this.renderLoadingSections()}
          </section>
        </main>
        ${this.renderFooter()}
      </div>
    `;

    this.bindHeader();
  }

  renderHeader() {
    return `
      <header class="topbar">
        <div class="topbar__inner">
          <div class="brand">
            <button class="brand__menu-button" type="button" aria-haspopup="menu" aria-expanded="false" aria-label="Open ${escapeHtml(defaultSiteName)} menu" data-board-menu-button>
              ${brandIcon}
              <span data-site-name>${escapeHtml(defaultSiteName)}</span>
            </button>
            <div class="brand-menu" role="menu" hidden data-board-menu>
              ${this.renderBoardMenu([])}
            </div>
          </div>
          <nav class="nav" aria-label="Primary navigation">
            ${this.renderUserNav()}
          </nav>
        </div>
      </header>
    `;
  }

  renderUserNav() {
    if (!this.session.loggedIn) {
      return `<a class="login-link" href="/login">login</a>`;
    }

    return `
      <div class="user-menu">
        <button class="icon-button" type="button" aria-haspopup="menu" aria-expanded="false" data-user-menu-button>
          ${userIcon}
          <span class="sr-only">Open user menu</span>
        </button>
        <div class="user-menu__panel" role="menu" hidden data-user-menu>
          <a role="menuitem" href="/profile">Profile</a>
          <a role="menuitem" href="/search">Search</a>
          <a role="menuitem" href="/logout">Exit</a>
        </div>
      </div>
    `;
  }

  bindHeader() {
    const boardButton = this.querySelector("[data-board-menu-button]");
    const boardMenu = this.querySelector("[data-board-menu]");
    const pageMask = this.querySelector("[data-page-mask]");
    if (boardButton && boardMenu) {
      const setBoardMenuOpen = (open) => {
        boardButton.setAttribute("aria-expanded", String(open));
        boardMenu.hidden = !open;
        if (pageMask) {
          pageMask.hidden = !open;
        }
        document.documentElement.style.setProperty(
          "--topbar-height",
          `${this.querySelector(".topbar")?.getBoundingClientRect().height ?? 0}px`,
        );
      };

      boardButton.addEventListener("click", (event) => {
        event.stopPropagation();
        const expanded = boardButton.getAttribute("aria-expanded") === "true";
        setBoardMenuOpen(!expanded);
      });

      boardMenu.addEventListener("click", (event) => {
        event.stopPropagation();
      });

      if (pageMask) {
        pageMask.addEventListener("click", () => {
          setBoardMenuOpen(false);
        });
      }

      document.addEventListener("click", () => {
        setBoardMenuOpen(false);
      });

      document.addEventListener("keydown", (event) => {
        if (event.key === "Escape") {
          setBoardMenuOpen(false);
          boardButton.focus();
        }
      });
    }

    const button = this.querySelector("[data-user-menu-button]");
    const menu = this.querySelector("[data-user-menu]");
    if (!button || !menu) {
      return;
    }

    button.addEventListener("click", () => {
      const expanded = button.getAttribute("aria-expanded") === "true";
      button.setAttribute("aria-expanded", String(!expanded));
      menu.hidden = expanded;
    });
  }

  renderFooter() {
    return `
      <footer class="footer">
        <div class="footer__inner">
          <span><span data-site-name>${escapeHtml(defaultSiteName)}</span> forum</span>
          <span>PostgreSQL-backed Rust web application</span>
          <span>&copy; ${new Date().getFullYear()} <span data-site-name>${escapeHtml(defaultSiteName)}</span></span>
        </div>
      </footer>
    `;
  }

  renderLoadingSections() {
    return [
      ["Recent announcement posts", sectionIcons.posts],
      ["Recent root posts", sectionIcons.posts],
      ["Recent original posts", sectionIcons.posts],
      ["Recent forward posts", sectionIcons.posts],
      ["New users", sectionIcons.users],
      ["Top point users", sectionIcons.users],
      ["Boards", sectionIcons.boards],
    ]
      .map(
        ([title, icon]) => `
          <section class="section" aria-labelledby="${this.sectionId(title)}">
            <div class="section__header">
              ${icon}
              <h2 id="${this.sectionId(title)}">${escapeHtml(title)}</h2>
            </div>
            <p class="section__state">Loading...</p>
          </section>
        `,
      )
      .join("");
  }

  async loadHome() {
    const dashboard = this.querySelector(".dashboard");
    try {
      const data = await getHome();
      this.applySiteName(siteNameFrom(data));
      this.applyBoardMenu(data.boards);
      dashboard.innerHTML = this.renderDashboard(data);
    } catch (error) {
      dashboard.innerHTML = `
        <section class="section section--wide">
          <h2>Unable to load forum data</h2>
          <p class="section__state">The page shell loaded, but the JSON API did not respond successfully.</p>
        </section>
      `;
      console.error(error);
    }
  }

  async loadBoard(boardId, page) {
    const dashboard = this.querySelector(".dashboard");
    try {
      const data = await getBoard(boardId, page);
      this.applySiteName(siteNameFrom(data));
      this.applyBoardMenu(data.boards || []);
      this.applyBoardIntro(data.board);
      dashboard.innerHTML = this.renderBoardPage(data);
    } catch (error) {
      dashboard.innerHTML = `
        <section class="section section--wide">
          <h2>Unable to load board data</h2>
          <p class="section__state">The page shell loaded, but the board JSON API did not respond successfully.</p>
        </section>
      `;
      console.error(error);
    }
  }

  applySiteName(siteName) {
    document.title = siteName;
    this.querySelectorAll("[data-site-name]").forEach((element) => {
      element.textContent = siteName;
    });

    const pageTitle = this.querySelector("#page-title");
    if (pageTitle) {
      pageTitle.textContent = siteName;
    }

    const brand = this.querySelector(".brand");
    if (brand) {
      const brandButton = brand.querySelector("[data-board-menu-button]");

      if (brandButton) {
        brandButton.setAttribute("aria-label", `Open ${siteName} menu`);
      }
    }
  }

  applyIntro(eyebrow, title, description) {
    const eyebrowElement = this.querySelector(".eyebrow");
    const titleElement = this.querySelector("#page-title");
    const descriptionElement = this.querySelector(".intro p:last-child");

    if (eyebrowElement) {
      eyebrowElement.textContent = eyebrow;
    }

    if (titleElement) {
      titleElement.textContent = title;
    }

    if (descriptionElement) {
      descriptionElement.textContent = description;
    }
  }

  applyBoardIntro(board) {
    const masters = board.master_names?.length ? board.master_names.join(", ") : "No board masters";
    this.applyIntro("Board", board.name, board.comment || "Post trees and board activity.");

    const intro = this.querySelector(".intro");
    if (!intro) {
      return;
    }

    intro.querySelector("[data-board-intro-extra]")?.remove();
    intro.insertAdjacentHTML(
      "beforeend",
      `
        <div class="intro__extra" data-board-intro-extra>
          <p class="item-card__meta">${meta([
            `Category: ${board.category_name}`,
            `Masters: ${masters}`,
          ])}</p>
          <div class="intro__metrics">
            ${this.renderMetric(board.post_count, "posts")}
            ${this.renderMetric(board.root_count ?? 0, "roots")}
          </div>
        </div>
      `,
    );
  }

  applyBoardMenu(boards) {
    const menu = this.querySelector("[data-board-menu]");
    if (menu) {
      menu.innerHTML = this.renderBoardMenu(boards || []);
    }
  }

  renderBoardMenu(boards) {
    const groups = groupedBoards(boards);

    return `
      <a class="brand-menu__portal" href="/" role="menuitem">Portal</a>
      ${
        groups.length
          ? groups
              .map(
                (group) => `
                  <section class="brand-menu__group" aria-labelledby="brand-menu-category-${escapeHtml(group.id)}">
                    <h2 id="brand-menu-category-${escapeHtml(group.id)}">
                      ${sectionIcons.boards}
                      <span>${escapeHtml(group.name)}</span>
                    </h2>
                    <div class="brand-menu__links">
                      ${group.boards
                        .map(
                          (board) => `
                            <a href="/board/${board.id}" role="menuitem">${escapeHtml(board.name)}</a>
                          `,
                        )
                        .join("")}
                    </div>
                  </section>
                `,
              )
              .join("")
          : `<p class="brand-menu__state">Boards loading...</p>`
      }
    `;
  }

  renderDashboard(data) {
    return `
      ${this.renderPostSection("Recent announcement posts", data.recent_announcement_posts)}
      ${this.renderPostSection("Recent root posts", data.recent_root_posts)}
      ${this.renderPostSection("Recent original posts", data.recent_original_posts)}
      ${this.renderPostSection("Recent forward posts", data.recent_forward_posts)}
      ${this.renderUserSection("New users", data.new_users, "Joined")}
      ${this.renderUserSection("Top point users", data.top_point_users, "Points")}
      ${this.renderBoardSection("Boards", data.boards)}
    `;
  }

  renderPostSection(title, posts) {
    return `
      <section class="section" aria-labelledby="${this.sectionId(title)}">
        <div class="section__header">
          ${sectionIcons.posts}
          <h2 id="${this.sectionId(title)}">${escapeHtml(title)}</h2>
        </div>
        <div class="item-list">
          ${
            posts.length
              ? posts.map((post) => this.renderPostCard(post)).join("")
              : `<p class="section__state">No posts.</p>`
          }
        </div>
      </section>
    `;
  }

  renderPostCard(post) {
    const board = post.board_name ? `#${post.board_name}` : null;
    const author = post.user_name || (post.user_id ? `user ${post.user_id}` : null);
    const type = postTypeLabels[post.post_type] || "Post";
    const typeIcon = postTypeIcons[post.post_type] || postTypeIcons[0];
    const typeClass = postTypeClasses[post.post_type] || postTypeClasses[0];
    const statusBar = renderPostStatusBar(post);

    return `
      <article class="item-card">
        <span class="item-card__icon item-card__icon--post ${typeClass}" title="${escapeHtml(type)}">${typeIcon}</span>
        <div class="item-card__content">
          <div class="item-card__title-row">
            <a class="item-card__title" href="/posts/${post.id}">${postTitle(post)}</a>
            ${statusBar}
          </div>
          <p class="item-card__meta">${meta([board, author, post.post_time])}</p>
          <p class="item-card__stats">${meta([
            `${post.reply_count ?? 0} replies`,
            `${post.access_count ?? 0} views`,
            `${post.point ?? 0} points`,
          ])}</p>
        </div>
      </article>
    `;
  }

  renderUserSection(title, users, label) {
    return `
      <section class="section" aria-labelledby="${this.sectionId(title)}">
        <div class="section__header">
          ${sectionIcons.users}
          <h2 id="${this.sectionId(title)}">${escapeHtml(title)}</h2>
        </div>
        <div class="item-list">
          ${
            users.length
              ? users.map((user) => this.renderUserCard(user, label)).join("")
              : `<p class="section__state">No users.</p>`
          }
        </div>
      </section>
    `;
  }

  renderUserCard(user, label) {
    const joined = user.reg_time || "date unknown";
    return `
      <article class="item-card item-card--compact user-card">
        <span class="item-card__icon">${userListIcon}</span>
        <div class="user-card__body">
          <a class="item-card__title" href="/users/${user.id}">${escapeHtml(user.name)}</a>
          <p class="item-card__meta">Joined: ${escapeHtml(joined)}</p>
        </div>
        <div class="user-card__metrics" aria-label="User statistics">
          ${this.renderMetric(user.post_count, "posts")}
          ${this.renderMetric(user.point ?? 0, "points")}
        </div>
      </article>
    `;
  }

  renderMetric(value, label) {
    return `
      <span class="metric-pill">
        <span class="metric-pill__value">${escapeHtml(value)}</span>
        <span class="metric-pill__label">${escapeHtml(label)}</span>
      </span>
    `;
  }

  renderBoardSection(title, boards) {
    const groups = groupedBoards(boards);

    return `
      <section class="section section--wide" aria-labelledby="${this.sectionId(title)}">
        <div class="section__header">
          ${sectionIcons.boards}
          <h2 id="${this.sectionId(title)}">${escapeHtml(title)}</h2>
        </div>
        <div class="board-groups">
          ${
            boards.length
              ? groups.map((group) => this.renderBoardCategory(group)).join("")
              : `<p class="section__state">No boards.</p>`
          }
        </div>
      </section>
    `;
  }

  renderBoardCategory(group) {
    return `
      <section class="board-category" aria-labelledby="board-category-${escapeHtml(group.id)}">
        <div class="board-category__header">
          ${sectionIcons.boards}
          <h3 id="board-category-${escapeHtml(group.id)}">${escapeHtml(group.name)}</h3>
        </div>
        <div class="board-grid">
          ${group.boards.map((board) => this.renderBoardCard(board)).join("")}
        </div>
      </section>
    `;
  }

  renderBoardCard(board) {
    return `
      <article class="board-card">
        <span class="item-card__icon">${boardListIcon}</span>
        <div class="board-card__body">
          <a class="item-card__title" href="/board/${board.id}">${escapeHtml(board.name)}</a>
          <p>${escapeHtml(board.comment || "")}</p>
        </div>
        <div class="board-card__metrics" aria-label="Board statistics">
          ${this.renderMetric(board.post_count, "posts")}
          ${this.renderMetric(board.root_count ?? 0, "roots")}
        </div>
      </article>
    `;
  }

  sectionId(title) {
    return title.toLowerCase().replaceAll(" ", "-");
  }

  renderBoardPage(data) {
    return `
      ${this.renderPager(data.pager, data.board.id)}
      ${this.renderPostTrees(data.trees)}
      ${this.renderPager(data.pager, data.board.id)}
    `;
  }

  renderPager(pager, boardId) {
    const page = Number(pager.page || 1);
    const totalPages = Number(pager.total_pages || 0);
    return `
      <nav class="pager section section--wide" aria-label="Board pagination">
        <a class="pager__button ${page <= 1 ? "is-disabled" : ""}" href="${this.boardPageHref(boardId, 1)}" aria-disabled="${page <= 1}">First</a>
        <a class="pager__button ${page <= 1 ? "is-disabled" : ""}" href="${this.boardPageHref(boardId, Math.max(1, page - 1))}" aria-disabled="${page <= 1}">Previous</a>
        <span class="pager__status">Page ${escapeHtml(page)} / ${escapeHtml(totalPages || 1)}</span>
        <a class="pager__button ${page >= totalPages ? "is-disabled" : ""}" href="${this.boardPageHref(boardId, Math.min(totalPages || 1, page + 1))}" aria-disabled="${page >= totalPages}">Next</a>
        <a class="pager__button ${page >= totalPages ? "is-disabled" : ""}" href="${this.boardPageHref(boardId, totalPages || 1)}" aria-disabled="${page >= totalPages}">Last</a>
      </nav>
    `;
  }

  boardPageHref(boardId, page) {
    return `/board/${encodeURIComponent(boardId)}?page=${encodeURIComponent(page)}`;
  }

  renderPostTrees(trees) {
    return trees.length
      ? trees.map((tree) => this.renderPostTree(tree)).join("")
      : `<section class="section section--wide"><p class="section__state">No posts.</p></section>`;
  }

  renderPostTree(tree) {
    return `
      <article class="post-tree-card section section--wide">
        ${tree.posts.map((post) => this.renderBoardPost(post)).join("")}
      </article>
    `;
  }

  renderBoardPost(post) {
    const author = post.user_name || (post.user_id ? `user ${post.user_id}` : null);
    const type = postTypeLabels[post.post_type] || "Post";
    const typeIcon = postTypeIcons[post.post_type] || postTypeIcons[0];
    const typeClass = postTypeClasses[post.post_type] || postTypeClasses[0];
    const statusBar = renderPostStatusBar(post);
    const level = Math.max(0, Number(post.level) || 0);
    const isRoot = level === 0;
    const metaParts = [
      author,
      post.post_time,
      `${post.access_count ?? 0} views`,
      `${post.point ?? 0} points`,
    ];

    if (isRoot && post.reply_time) {
      metaParts.push(`Latest reply: ${post.reply_time}`);
    }

    return `
      <div class="item-card board-post" style="--post-indent: ${Math.min(level, 8)}">
        <span class="item-card__icon item-card__icon--post ${typeClass}" title="${escapeHtml(type)}">${typeIcon}</span>
        <div class="item-card__content">
          <div class="item-card__title-row">
            <a class="item-card__title" href="/posts/${post.id}">${postTitle(post)}</a>
            ${statusBar}
          </div>
          <p class="item-card__meta">${meta(metaParts)}</p>
        </div>
      </div>
    `;
  }
}

customElements.define("dogn-app-shell", DognAppShell);
