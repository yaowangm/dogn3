const defaultHeaders = {
  "Accept": "application/json",
};

class ApiError extends Error {
  constructor(status, body = null) {
    super(body?.error?.message || `Request failed: ${status}`);
    this.status = status;
    this.body = body;
  }
}

async function getJson(path, options = {}) {
  const response = await fetch(path, {
    cache: "no-store",
    ...options,
    headers: {
      ...defaultHeaders,
      ...options.headers,
    },
  });

  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new ApiError(response.status, body);
  }

  return response.json();
}

async function postJson(path, body = {}) {
  return getJson(path, {
    method: "POST",
    headers: { "Content-Type": "application/json", "X-Dogn-Request": "fetch" },
    body: JSON.stringify(body),
  });
}

function getHome() {
  return getJson("/api/home");
}

function getBoard(boardId, page = 1) {
  return getJson(`/api/boards/${encodeURIComponent(boardId)}?page=${encodeURIComponent(page)}`);
}

function getPost(postId) {
  return getJson(`/api/posts/${encodeURIComponent(postId)}`);
}

function getPostList(postId) {
  return getJson(`/api/post_lists/${encodeURIComponent(postId)}`);
}

function getPostPrint(postId) {
  return getJson(`/api/post_prints/${encodeURIComponent(postId)}`);
}

function getPostEditor(params) {
  return getJson(`/api/post_upd?${params.toString()}`);
}

function getUser(userId, activity = "original", page = 1) {
  return getJson(
    `/api/users/${encodeURIComponent(userId)}?activity=${encodeURIComponent(activity)}&page=${encodeURIComponent(page)}`,
  );
}

function getUserList(query = "", role = "", order = "id_desc", page = 1) {
  const params = new URLSearchParams({ query, role, order, page: String(page) });
  return getJson(`/api/users?${params.toString()}`);
}

function getSiteManager() {
  return getJson("/api/site_manager");
}

function getSession() {
  return getJson("/api/auth/session");
}

function submitLogin(name, password) {
  return postJson("/api/auth/login", { name, password });
}

function submitLogout() {
  return postJson("/api/auth/logout");
}

function submitPasswordChange(userId, currentPassword, newPassword, confirmPassword) {
  return postJson(`/api/users/${encodeURIComponent(userId)}/password`, {
    current_password: currentPassword || null,
    new_password: newPassword,
    confirm_password: confirmPassword,
  });
}

function submitStatisticsRecalculation(userId) {
  return postJson(`/api/users/${encodeURIComponent(userId)}/statistics/recalculate`);
}

function submitRoleChange(userId, level) {
  return postJson(`/api/users/${encodeURIComponent(userId)}/role`, { level });
}

function submitProfileUpdate(userId, email, intro) {
  return postJson(`/api/users/${encodeURIComponent(userId)}/profile`, { email, intro });
}

function submitUserCreation(values) {
  return postJson("/api/users", values);
}

function submitCategoryUpdate(categoryId, values) {
  return postJson(`/api/site_manager/categories/${encodeURIComponent(categoryId)}`, values);
}

function submitCategoryCreation(values) {
  return postJson("/api/site_manager/categories", values);
}

function submitCategoryDeletion(categoryId) {
  return postJson(`/api/site_manager/categories/${encodeURIComponent(categoryId)}/delete`);
}

function submitBoardUpdate(boardId, values) {
  return postJson(`/api/site_manager/boards/${encodeURIComponent(boardId)}`, values);
}

function submitBoardCreation(values) {
  return postJson("/api/site_manager/boards", values);
}

function submitBoardDeletion(boardId) {
  return postJson(`/api/site_manager/boards/${encodeURIComponent(boardId)}/delete`);
}

function submitBoardMasterAddition(boardId, userId) {
  return postJson(`/api/site_manager/boards/${encodeURIComponent(boardId)}/masters`, {
    user_id: userId,
  });
}

function submitBoardMasterRemoval(boardId, userId) {
  return postJson(
    `/api/site_manager/boards/${encodeURIComponent(boardId)}/masters/${encodeURIComponent(userId)}/remove`,
  );
}

function submitBoardStatisticsRecalculation() {
  return postJson("/api/site_manager/boards/statistics/recalculate");
}

function submitPostSave(values) {
  return postJson("/api/post_upd", values);
}

function submitPostDeletion(postId) {
  return postJson(`/api/posts/${encodeURIComponent(postId)}/delete`);
}

async function submitPostImageUpload(postId, file) {
  const response = await fetch(`/api/posts/${encodeURIComponent(postId)}/image`, {
    method: "POST",
    cache: "no-store",
    headers: {
      ...defaultHeaders,
      "Content-Type": file.type,
      "X-Dogn-Request": "fetch",
    },
    body: file,
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new ApiError(response.status, body);
  }
  return response.json();
}

function localPagePath() {
  return `${window.location.pathname}${window.location.search}${window.location.hash}`;
}

function validReturnPath(value) {
  if (!value) {
    return null;
  }

  try {
    const url = new URL(value, window.location.origin);
    if (url.origin !== window.location.origin || /^\/login\/?$/.test(url.pathname)) {
      return null;
    }

    return `${url.pathname}${url.search}${url.hash}`;
  } catch (_error) {
    return null;
  }
}

function previousPageOrDefault() {
  const requested = new URLSearchParams(window.location.search).get("return_to");
  const fromQuery = validReturnPath(requested);
  if (fromQuery) {
    return fromQuery;
  }

  return validReturnPath(document.referrer) || "/";
}

function formatFileSizeLimit(bytes) {
  const size = Number(bytes) || 0;
  if (size >= 1024 * 1024 && size % (1024 * 1024) === 0) {
    return `${size / (1024 * 1024)} MB`;
  }
  if (size >= 1024 && size % 1024 === 0) {
    return `${size / 1024} KB`;
  }
  return `${size} bytes`;
}

function secureRandomIndex(length) {
  const limit = Math.floor(0x100000000 / length) * length;
  const values = new Uint32Array(1);
  do {
    window.crypto.getRandomValues(values);
  } while (values[0] >= limit);
  return values[0] % length;
}

function generateSuggestedPassword() {
  const letters = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  const digits = "23456789";
  const symbols = "!#$%&*+-=?@_";
  const characters = [
    letters[secureRandomIndex(letters.length)],
    digits[secureRandomIndex(digits.length)],
    symbols[secureRandomIndex(symbols.length)],
  ];
  const all = letters + digits + symbols;
  while (characters.length < 16) {
    characters.push(all[secureRandomIndex(all.length)]);
  }
  for (let index = characters.length - 1; index > 0; index -= 1) {
    const replacement = secureRandomIndex(index + 1);
    [characters[index], characters[replacement]] = [characters[replacement], characters[index]];
  }
  return characters.join("");
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

const replyIcon = `
  <svg class="reply-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
    <path d="M9 5l9 7-9 7z" />
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

const userMenuIcons = {
  profile: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <circle cx="12" cy="8" r="3" />
      <path d="M6 19c1-2.5 3-3.8 6-3.8s5 1.3 6 3.8" />
    </svg>
  `,
  users: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <circle cx="9" cy="8" r="3" />
      <path d="M3.8 18c.9-2.6 2.7-3.9 5.2-3.9s4.3 1.3 5.2 3.9" />
      <path d="M15.5 5.8a3 3 0 0 1 0 5.1" />
      <path d="M16 14.2c2 .4 3.4 1.7 4.1 3.8" />
    </svg>
  `,
  addUser: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <circle cx="9" cy="8" r="3" />
      <path d="M3.8 18c.9-2.6 2.7-3.9 5.2-3.9s4.3 1.3 5.2 3.9" />
      <path d="M18 9v8" />
      <path d="M14 13h8" />
    </svg>
  `,
  settings: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <circle cx="12" cy="12" r="3" />
      <path d="M12 3v4" />
      <path d="M12 17v4" />
      <path d="M3 12h4" />
      <path d="M17 12h4" />
      <path d="M5.6 5.6l2.8 2.8" />
      <path d="M15.6 15.6l2.8 2.8" />
      <path d="M18.4 5.6l-2.8 2.8" />
      <path d="M8.4 15.6l-2.8 2.8" />
    </svg>
  `,
  search: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <circle cx="10.5" cy="10.5" r="5.5" />
      <path d="M15 15l4.5 4.5" />
    </svg>
  `,
  exit: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M10 4H5v16h5" />
      <path d="M13 8l4 4-4 4" />
      <path d="M8 12h9" />
    </svg>
  `,
};

const userActionIcons = {
  password: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <rect x="5" y="10" width="14" height="10" rx="2" />
      <path d="M8 10V7.5a4 4 0 0 1 8 0V10" />
      <path d="M12 14v2" />
    </svg>
  `,
  calculate: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M12 4v4" />
      <path d="M12 16v4" />
      <path d="M4 12h4" />
      <path d="M16 12h4" />
      <path d="M6.5 6.5l2.8 2.8" />
      <path d="M14.7 14.7l2.8 2.8" />
      <path d="M17.5 6.5l-2.8 2.8" />
      <path d="M9.3 14.7l-2.8 2.8" />
    </svg>
  `,
  regenerate: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M20 11a8 8 0 0 0-13.5-5.8L4 8" />
      <path d="M4 4v4h4" />
      <path d="M4 13a8 8 0 0 0 13.5 5.8L20 16" />
      <path d="M20 20v-4h-4" />
    </svg>
  `,
  copy: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <rect x="8" y="8" width="11" height="12" rx="2" />
      <path d="M6 16H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v1" />
    </svg>
  `,
  role: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M12 3l7 3v5c0 4.7-2.6 8-7 10-4.4-2-7-5.3-7-10V6z" />
      <path d="M9 12l2 2 4-4" />
    </svg>
  `,
  update: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M4 20h4L19 9l-4-4L4 16z" />
      <path d="M13.5 6.5l4 4" />
      <path d="M4 20h16" />
    </svg>
  `,
};

const postMetaIcons = {
  board: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <rect x="5" y="5" width="14" height="14" rx="2" />
      <path d="M9 9h6" />
      <path d="M9 13h6" />
      <path d="M9 17h3" />
    </svg>
  `,
  author: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <circle cx="12" cy="8" r="3" />
      <path d="M6 19c1-1.9 3-3 6-3s5 1.1 6 3" />
    </svg>
  `,
  time: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <circle cx="12" cy="12" r="8" />
      <path d="M12 7v5l3 2" />
    </svg>
  `,
  size: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M7 4h7l3 3v13H7z" />
      <path d="M14 4v4h4" />
    </svg>
  `,
  views: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M3.5 12s3-6 8.5-6 8.5 6 8.5 6-3 6-8.5 6-8.5-6-8.5-6z" />
      <circle cx="12" cy="12" r="2.5" />
    </svg>
  `,
  points: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M12 19s-7-4.2-7-9.2A3.9 3.9 0 0 1 12 7.4a3.9 3.9 0 0 1 7 2.4c0 5-7 9.2-7 9.2z" />
    </svg>
  `,
  replies: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M5 6h14v10H9l-4 3z" />
    </svg>
  `,
  replyTime: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M5 6h14v10H9l-4 3z" />
      <path d="M12 9v4l2 1" />
    </svg>
  `,
  network: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <circle cx="12" cy="12" r="8" />
      <path d="M4 12h16" />
      <path d="M12 4c2.5 2.4 3.6 5.1 3.6 8S14.5 17.6 12 20" />
      <path d="M12 4c-2.5 2.4-3.6 5.1-3.6 8S9.5 17.6 12 20" />
    </svg>
  `,
  email: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <rect x="4" y="6" width="16" height="12" rx="2" />
      <path d="M5 8l7 5 7-5" />
    </svg>
  `,
};

const postActionIcons = {
  link: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M10 13a4 4 0 0 0 5.7.2l2.6-2.6a4 4 0 1 0-5.7-5.7L11 6.5" />
      <path d="M14 11a4 4 0 0 0-5.7-.2l-2.6 2.6a4 4 0 1 0 5.7 5.7L13 17.5" />
    </svg>
  `,
  list: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M8 6h11" />
      <path d="M8 12h11" />
      <path d="M8 18h11" />
      <circle cx="4.5" cy="6" r="1" />
      <circle cx="4.5" cy="12" r="1" />
      <circle cx="4.5" cy="18" r="1" />
    </svg>
  `,
  print: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M7 9V4h10v5" />
      <path d="M7 17H5V10h14v7h-2" />
      <path d="M7 14h10v6H7z" />
    </svg>
  `,
  reply: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M9 8l-5 4 5 4" />
      <path d="M4 12h9c4 0 6 2 7 6" />
    </svg>
  `,
  add: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M12 5v14" />
      <path d="M5 12h14" />
    </svg>
  `,
  delete: `
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <path d="M4 7h16" />
      <path d="M9 7V4h6v3" />
      <path d="M7 7l1 13h8l1-13" />
      <path d="M10 11v5" />
      <path d="M14 11v5" />
    </svg>
  `,
  edit: userActionIcons.update,
  externalImage: attachmentIcons.image,
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

  if (post.has_link || safeResourceUrl(post.link_url)) {
    icons.push(`<span title="Has related link">${postActionIcons.link}</span>`);
  }

  if (post.has_image || post.image_url) {
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

function safeResourceUrl(value) {
  if (!value) {
    return null;
  }

  const stringValue = String(value).trim();
  if (stringValue.startsWith("/") && !stringValue.startsWith("//")) {
    return stringValue;
  }

  try {
    const url = new URL(stringValue);
    return ["http:", "https:"].includes(url.protocol) ? url.href : null;
  } catch (_error) {
    return null;
  }
}

function isLocalResourceUrl(value) {
  return value?.startsWith("/") && !value.startsWith("//");
}

function postImageResource(value) {
  if (!value) {
    return null;
  }

  const stringValue = String(value).trim();
  const resourceUrl = safeResourceUrl(stringValue);
  if (resourceUrl && !isLocalResourceUrl(resourceUrl)) {
    return { url: resourceUrl, local: false };
  }

  if (
    !stringValue ||
    stringValue.startsWith("//") ||
    stringValue.includes("\\") ||
    /^[a-z][a-z\d+.-]*:/i.test(stringValue)
  ) {
    return null;
  }

  let relativePath = stringValue.replace(/^\/+/, "");
  if (relativePath.startsWith("images/")) {
    relativePath = relativePath.slice("images/".length);
  }

  const pathSegments = relativePath.split("/");
  if (pathSegments.some((segment) => !segment || segment === "." || segment === "..")) {
    return null;
  }

  return {
    url: `/images/${pathSegments.map((segment) => encodeURIComponent(segment)).join("/")}`,
    local: true,
  };
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
    const postPrintId = this.currentPostPrintId();
    if (postPrintId) {
      this.renderPrintShell();
      this.loadPostPrint(postPrintId);
      return;
    }

    this.initialize();
  }

  async initialize() {
    try {
      const session = await getSession();
      this.session = {
        loggedIn: session.authenticated,
        user: session.user,
      };
      this.render();
      this.applySiteName(siteNameFrom(session));
    } catch (error) {
      this.render();
      console.error(error);
    }

    if (this.isLoginPage()) {
      this.loadLogin();
      return;
    }

    this.loadCurrentPage();
  }

  loadCurrentPage() {
    if (this.isPostUpdatePage()) {
      this.loadPostUpdate();
      return;
    }

    const postListId = this.currentPostListId();
    if (postListId) {
      this.loadPostList(postListId);
      return;
    }

    const postId = this.currentPostId();
    if (postId) {
      this.loadPost(postId);
      return;
    }

    const boardId = this.currentBoardId();
    if (boardId) {
      this.loadBoard(boardId, this.currentPage());
      return;
    }

    const userId = this.currentUserId();
    if (userId) {
      this.loadUser(userId, this.currentActivity(), this.currentPage());
      return;
    }

    if (this.isUserListPage()) {
      this.loadUserList(this.currentUserSearch(), this.currentUserRole(), this.currentUserOrder(), this.currentPage());
      return;
    }

    if (this.isUserAddPage()) {
      this.loadUserAdd();
      return;
    }

    if (this.isSiteManagerPage()) {
      this.loadSiteManager();
      return;
    }

    this.loadHome();
  }

  currentBoardId() {
    const match = window.location.pathname.match(/^\/board\/(\d+)\/?$/);
    return match?.[1] || null;
  }

  currentPostId() {
    const match = window.location.pathname.match(/^\/post\/(\d+)\/?$/);
    return match?.[1] || null;
  }

  currentPostListId() {
    const match = window.location.pathname.match(/^\/post_list\/(\d+)\/?$/);
    return match?.[1] || null;
  }

  currentPostPrintId() {
    const match = window.location.pathname.match(/^\/post_print\/(\d+)\/?$/);
    return match?.[1] || null;
  }

  currentUserId() {
    const match = window.location.pathname.match(/^\/user\/(\d+)\/?$/);
    return match?.[1] || null;
  }

  isUserListPage() {
    return /^\/user_list\/?$/.test(window.location.pathname);
  }

  isUserAddPage() {
    return /^\/user_add\/?$/.test(window.location.pathname);
  }

  isSiteManagerPage() {
    return /^\/site_mgr\/?$/.test(window.location.pathname);
  }

  isPostUpdatePage() {
    return /^\/post_upd\/?$/.test(window.location.pathname);
  }

  isLoginPage() {
    return /^\/login\/?$/.test(window.location.pathname);
  }

  currentPage() {
    const page = Number(new URLSearchParams(window.location.search).get("page") || "1");
    return Number.isInteger(page) && page > 0 ? page : 1;
  }

  currentActivity() {
    const activity = new URLSearchParams(window.location.search).get("activity");
    return ["favorites", "signatures"].includes(activity) ? activity : "original";
  }

  currentUserSearch() {
    return new URLSearchParams(window.location.search).get("query") || "";
  }

  currentUserOrder() {
    return new URLSearchParams(window.location.search).get("order") === "id_asc"
      ? "id_asc"
      : "id_desc";
  }

  currentUserRole() {
    const role = new URLSearchParams(window.location.search).get("role") || "";
    return ["active", "0", "1", "5", "10"].includes(role) ? role : "";
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

  renderPrintShell() {
    this.innerHTML = `
      <main class="print-page" id="main-content">
        <p class="print-page__state">Loading post...</p>
      </main>
    `;
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
      const returnTo = encodeURIComponent(localPagePath());
      return `<a class="login-link" href="/login?return_to=${returnTo}">login</a>`;
    }

    const userName = this.session.user?.name || "user";
    const userListLink =
      Number(this.session.user?.level || 0) >= 10
        ? `<a role="menuitem" href="/user_list">${userMenuIcons.users}<span>User list</span></a>`
        : "";
    const userAddLink =
      Number(this.session.user?.level || 0) >= 10
        ? `<a role="menuitem" href="/user_add">${userMenuIcons.addUser}<span>Add user</span></a>`
        : "";
    const siteManagerLink =
      Number(this.session.user?.level || 0) >= 10
        ? `<a role="menuitem" href="/site_mgr">${userMenuIcons.settings}<span>Site manager</span></a>`
        : "";
    return `
      <div class="user-menu">
        <button class="user-menu__trigger" type="button" aria-haspopup="menu" aria-expanded="false" aria-label="Open ${escapeHtml(userName)} menu" data-user-menu-button>
          ${userIcon}
          <span>${escapeHtml(userName)}</span>
        </button>
        <div class="user-menu__panel" role="menu" hidden data-user-menu>
          <a role="menuitem" href="/user/${encodeURIComponent(this.session.user.id)}">${userMenuIcons.profile}<span>Profile</span></a>
          ${userAddLink}
          ${siteManagerLink}
          ${userListLink}
          <a role="menuitem" href="/search">${userMenuIcons.search}<span>Search</span></a>
          <button class="user-menu__action" type="button" role="menuitem" data-logout>${userMenuIcons.exit}<span>Exit</span></button>
        </div>
      </div>
    `;
  }

  bindHeader() {
    const boardButton = this.querySelector("[data-board-menu-button]");
    const boardMenu = this.querySelector("[data-board-menu]");
    const userButton = this.querySelector("[data-user-menu-button]");
    const userMenu = this.querySelector("[data-user-menu]");
    const pageMask = this.querySelector("[data-page-mask]");
    const isMenuOpen = (button) => button?.getAttribute("aria-expanded") === "true";
    const syncPageMask = () => {
      if (pageMask) {
        pageMask.hidden = !isMenuOpen(boardButton) && !isMenuOpen(userButton);
      }
    };
    const setBoardMenuOpen = (open) => {
      if (boardButton && boardMenu) {
        boardButton.setAttribute("aria-expanded", String(open));
        boardMenu.hidden = !open;
        document.documentElement.style.setProperty(
          "--topbar-height",
          `${this.querySelector(".topbar")?.getBoundingClientRect().height ?? 0}px`,
        );
      }
      syncPageMask();
    };
    const setUserMenuOpen = (open) => {
      if (userButton && userMenu) {
        userButton.setAttribute("aria-expanded", String(open));
        userMenu.hidden = !open;
      }
      syncPageMask();
    };
    const closeMenus = () => {
      setBoardMenuOpen(false);
      setUserMenuOpen(false);
    };

    if (boardButton && boardMenu) {
      boardButton.addEventListener("click", (event) => {
        event.stopPropagation();
        const expanded = isMenuOpen(boardButton);
        setUserMenuOpen(false);
        setBoardMenuOpen(!expanded);
      });

      boardMenu.addEventListener("click", (event) => {
        event.stopPropagation();
      });
    }

    if (userButton && userMenu) {
      userButton.addEventListener("click", (event) => {
        event.stopPropagation();
        const expanded = isMenuOpen(userButton);
        setBoardMenuOpen(false);
        setUserMenuOpen(!expanded);
      });

      userMenu.addEventListener("click", (event) => {
        event.stopPropagation();
      });

      this.querySelector("[data-logout]")?.addEventListener("click", async () => {
        try {
          await submitLogout();
          window.location.assign(validReturnPath(localPagePath()) || "/");
        } catch (error) {
          console.error(error);
        }
      });
    }

    pageMask?.addEventListener("click", closeMenus);
    document.addEventListener("click", closeMenus);
    document.addEventListener("keydown", (event) => {
      if (event.key !== "Escape") {
        return;
      }

      const focusTarget = isMenuOpen(userButton)
        ? userButton
        : isMenuOpen(boardButton)
          ? boardButton
          : null;
      closeMenus();
      focusTarget?.focus();
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

  async loadLogin() {
    const intro = this.querySelector(".intro");
    const dashboard = this.querySelector(".dashboard");
    if (intro) {
      intro.hidden = true;
    }

    this.applyPageTitle("Login", document.querySelector("[data-site-name]")?.textContent || defaultSiteName);
    dashboard.setAttribute("aria-label", "Login");
    dashboard.innerHTML = `
      <section class="login-panel section section--wide" aria-labelledby="login-title">
        <div class="login-panel__header">
          <h1 id="login-title">Login</h1>
          <p>Enter your forum user name and password.</p>
        </div>
        <form class="login-form" data-login-form>
          <label class="login-field">
            <span>User name</span>
            <input type="text" name="name" autocomplete="username" required autofocus>
          </label>
          <label class="login-field">
            <span>Password</span>
            <input type="password" name="password" autocomplete="current-password" required>
          </label>
          <p class="login-form__error" data-login-error hidden>Invalid user name or password.</p>
          <button class="login-submit" type="submit">Login</button>
        </form>
      </section>
    `;
    this.bindLogin();

    try {
      const data = await getHome();
      this.applySiteName(siteNameFrom(data));
      this.applyPageTitle("Login", siteNameFrom(data));
      this.applyBoardMenu(data.boards || []);
    } catch (error) {
      console.error(error);
    }
  }

  bindLogin() {
    const form = this.querySelector("[data-login-form]");
    const error = this.querySelector("[data-login-error]");
    if (!form || !error) {
      return;
    }

    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      const button = form.querySelector("button[type=submit]");
      const fields = new FormData(form);
      error.hidden = true;
      button.disabled = true;

      try {
        await submitLogin(String(fields.get("name") || ""), String(fields.get("password") || ""));
        window.location.assign(previousPageOrDefault());
      } catch (_error) {
        error.hidden = false;
        button.disabled = false;
      }
    });
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
      const siteName = siteNameFrom(data);
      this.applySiteName(siteName);
      this.applyPageTitle(data.board?.name, siteName);
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

  async loadUser(userId, activity, page) {
    const intro = this.querySelector(".intro");
    const dashboard = this.querySelector(".dashboard");
    if (intro) {
      intro.hidden = true;
    }
    dashboard.innerHTML = `
      <section class="section section--wide">
        <p class="section__state">Loading user...</p>
      </section>
    `;

    try {
      const data = await getUser(userId, activity, page);
      const siteName = siteNameFrom(data);
      this.applySiteName(siteName);
      this.applyPageTitle(data.user.name, siteName);
      this.applyBoardMenu(data.boards || []);
      dashboard.setAttribute("aria-label", "User profile");
      dashboard.innerHTML = this.renderUserPage(data);
      this.bindUserActions(data);
    } catch (error) {
      const notFound = error instanceof ApiError && error.status === 404;
      dashboard.innerHTML = notFound
        ? `
          <section class="section section--wide post-unavailable">
            <h2>User unavailable</h2>
            <p class="section__state">This user does not exist.</p>
          </section>
        `
        : `
          <section class="section section--wide">
            <h2>Unable to load user data</h2>
            <p class="section__state">The page shell loaded, but the user JSON API did not respond successfully.</p>
          </section>
        `;
      console.error(error);
    }
  }

  async loadUserList(query, role, order, page) {
    const intro = this.querySelector(".intro");
    const dashboard = this.querySelector(".dashboard");
    if (intro) {
      intro.hidden = true;
    }
    this.applyPageTitle("User list", document.querySelector("[data-site-name]")?.textContent || defaultSiteName);
    dashboard.setAttribute("aria-label", "User list");
    dashboard.innerHTML = `
      <section class="section section--wide">
        <p class="section__state">Loading users...</p>
      </section>
    `;

    try {
      const data = await getUserList(query, role, order, page);
      const siteName = siteNameFrom(data);
      this.applySiteName(siteName);
      this.applyPageTitle("User list", siteName);
      this.applyBoardMenu(data.boards || []);
      dashboard.innerHTML = this.renderUserListPage(data);
    } catch (error) {
      const denied = error instanceof ApiError && [401, 403].includes(error.status);
      dashboard.innerHTML = denied
        ? `
          <section class="section section--wide post-unavailable">
            <h2>Administrator access required</h2>
            <p class="section__state">This page is available only to administrators.</p>
          </section>
        `
        : `
          <section class="section section--wide">
            <h2>Unable to load user list</h2>
            <p class="section__state">The page shell loaded, but the user-list JSON API did not respond successfully.</p>
          </section>
        `;
      console.error(error);
    }
  }

  loadUserAdd() {
    const intro = this.querySelector(".intro");
    const dashboard = this.querySelector(".dashboard");
    if (intro) {
      intro.hidden = true;
    }
    const siteName = document.querySelector("[data-site-name]")?.textContent || defaultSiteName;
    this.applyPageTitle("Add user", siteName);
    dashboard.setAttribute("aria-label", "Add user");

    if (!this.session.loggedIn || Number(this.session.user?.level || 0) < 10) {
      dashboard.innerHTML = `
        <section class="section section--wide post-unavailable">
          <h2>Administrator access required</h2>
          <p class="section__state">This page is available only to administrators.</p>
        </section>
      `;
      return;
    }

    dashboard.innerHTML = this.renderUserAddPage();
    this.bindUserAddActions();
  }

  async loadSiteManager() {
    const intro = this.querySelector(".intro");
    const dashboard = this.querySelector(".dashboard");
    if (intro) {
      intro.hidden = true;
    }
    this.applyPageTitle("Site manager", document.querySelector("[data-site-name]")?.textContent || defaultSiteName);
    dashboard.setAttribute("aria-label", "Site manager");
    dashboard.innerHTML = `
      <section class="section section--wide">
        <p class="section__state">Loading site management data...</p>
      </section>
    `;

    try {
      const data = await getSiteManager();
      const siteName = siteNameFrom(data);
      this.applySiteName(siteName);
      this.applyPageTitle("Site manager", siteName);
      this.applyBoardMenu(data.navigation_boards || []);
      dashboard.innerHTML = this.renderSiteManagerPage(data);
      this.bindSiteManagerActions();
    } catch (error) {
      const denied = error instanceof ApiError && [401, 403].includes(error.status);
      dashboard.innerHTML = denied
        ? `
          <section class="section section--wide post-unavailable">
            <h2>Administrator access required</h2>
            <p class="section__state">This page is available only to administrators.</p>
          </section>
        `
        : `
          <section class="section section--wide">
            <h2>Unable to load site manager</h2>
            <p class="section__state">The page shell loaded, but the site-manager JSON API did not respond successfully.</p>
          </section>
        `;
      console.error(error);
    }
  }

  async loadPost(postId) {
    const intro = this.querySelector(".intro");
    const dashboard = this.querySelector(".dashboard");
    if (intro) {
      intro.hidden = true;
    }
    dashboard.innerHTML = `
      <section class="section section--wide">
        <p class="section__state">Loading post...</p>
      </section>
    `;

    try {
      const data = await getPost(postId);
      const siteName = siteNameFrom(data);
      this.applySiteName(siteName);
      this.applyPageTitle(data.post?.subject || "(untitled)", siteName);
      this.applyBoardMenu(data.boards || []);
      dashboard.setAttribute("aria-label", "Post detail");
      dashboard.innerHTML = this.renderPostPage(data);
      this.bindPostActions(data);
    } catch (error) {
      const notFound = error instanceof ApiError && error.status === 404;
      dashboard.innerHTML = notFound
        ? `
          <section class="section section--wide post-unavailable">
            <h2>Post unavailable</h2>
            <p class="section__state">This post does not exist or has been deleted.</p>
          </section>
        `
        : `
          <section class="section section--wide">
            <h2>Unable to load post data</h2>
            <p class="section__state">The page shell loaded, but the post JSON API did not respond successfully.</p>
          </section>
        `;
      console.error(error);
    }
  }

  async loadPostUpdate() {
    const intro = this.querySelector(".intro");
    const dashboard = this.querySelector(".dashboard");
    if (intro) {
      intro.hidden = true;
    }
    const params = new URLSearchParams(window.location.search);
    dashboard.setAttribute("aria-label", "Post editor");
    dashboard.innerHTML = `
      <section class="section section--wide">
        <p class="section__state">Loading post editor...</p>
      </section>
    `;

    try {
      const data = await getPostEditor(params);
      const siteName = siteNameFrom(data);
      const heading =
        data.mode === "reply"
          ? `Reply to: ${data.parent?.subject || "(untitled)"}`
          : data.mode === "create"
            ? "Add post"
            : "Update post";
      this.applySiteName(siteName);
      this.applyPageTitle(heading, siteName);
      this.applyBoardMenu(data.boards || []);
      dashboard.innerHTML = this.renderPostEditor(data);
      this.bindPostEditor(data);
    } catch (error) {
      const loginRequired = error instanceof ApiError && error.status === 401;
      const forbidden = error instanceof ApiError && error.status === 403;
      const replyClosed =
        error instanceof ApiError && error.body?.error?.code === "reply_closed";
      dashboard.innerHTML = loginRequired
        ? `
          <section class="section section--wide post-unavailable">
            <h2>Login required</h2>
            <p class="section__state">Login is required to write a post.</p>
            <a class="post-editor__login" href="/login?return_to=${encodeURIComponent(localPagePath())}">Login</a>
          </section>
        `
          : forbidden
          ? `
              <section class="section section--wide post-unavailable">
                <h2>Update not permitted</h2>
                <p class="section__state">You do not have permission to update this post.</p>
              </section>
          `
          : replyClosed
            ? `
              <section class="section section--wide post-unavailable">
                <h2>Replies closed</h2>
                <p class="section__state">This post is no longer open for replies.</p>
              </section>
            `
          : `
            <section class="section section--wide">
              <h2>Unable to load post editor</h2>
              <p class="section__state">The editor target is unavailable.</p>
            </section>
          `;
      console.error(error);
    }
  }

  async loadPostList(postId) {
    const intro = this.querySelector(".intro");
    const dashboard = this.querySelector(".dashboard");
    if (intro) {
      intro.hidden = true;
    }
    dashboard.innerHTML = `
      <section class="section section--wide">
        <p class="section__state">Loading post tree...</p>
      </section>
    `;

    try {
      const data = await getPostList(postId);
      const selectedPost =
        data.posts.find((post) => Number(post.id) === Number(data.selected_post_id)) || data.posts[0];
      const siteName = siteNameFrom(data);
      this.applySiteName(siteName);
      this.applyPageTitle(selectedPost?.subject || "(untitled)", siteName);
      this.applyBoardMenu(data.boards || []);
      dashboard.setAttribute("aria-label", "Post list");
      dashboard.innerHTML = this.renderPostListPage(data);
      this.revealPostListSelection(selectedPost);
    } catch (error) {
      const notFound = error instanceof ApiError && error.status === 404;
      dashboard.innerHTML = notFound
        ? `
          <section class="section section--wide post-unavailable">
            <h2>Post unavailable</h2>
            <p class="section__state">This post does not exist or has been deleted.</p>
          </section>
        `
        : `
          <section class="section section--wide">
            <h2>Unable to load post tree</h2>
            <p class="section__state">The page shell loaded, but the post list JSON API did not respond successfully.</p>
          </section>
        `;
      console.error(error);
    }
  }

  async loadPostPrint(postId) {
    const page = this.querySelector(".print-page");
    try {
      const data = await getPostPrint(postId);
      const siteName = siteNameFrom(data);
      const subject = data.post?.subject || "(untitled)";
      document.title = `${subject} - ${siteName} - Print`;
      page.innerHTML = this.renderPrintPost(data);
    } catch (error) {
      const notFound = error instanceof ApiError && error.status === 404;
      page.innerHTML = notFound
        ? `
          <section class="print-page__message">
            <h1>Post unavailable</h1>
            <p>This post does not exist or has been deleted.</p>
          </section>
        `
        : `
          <section class="print-page__message">
            <h1>Unable to load post</h1>
            <p>The print version could not be loaded.</p>
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

  applyPageTitle(pageName, siteName) {
    const name = String(pageName || "").trim();
    document.title = name ? `${name} - ${siteName}` : siteName;
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
    const masters = board.master_users?.length
      ? board.master_users.map((master) => master.name).join(", ")
      : "No board masters";
    this.applyIntro("Board", board.name, board.comment || "Post trees and board activity.");

    const intro = this.querySelector(".intro");
    if (!intro) {
      return;
    }

    const titleElement = intro.querySelector("#page-title");
    if (titleElement) {
      const titleLink = document.createElement("a");
      titleLink.href = `/board/${encodeURIComponent(board.id)}`;
      titleLink.textContent = board.name;
      titleElement.replaceChildren(titleLink);
    }

    intro.querySelector("[data-board-intro-metrics]")?.remove();
    intro.querySelector("[data-board-intro-extra]")?.remove();
    intro.insertAdjacentHTML(
      "beforeend",
      `
        <div class="intro__metrics" data-board-intro-metrics>
          ${this.renderMetric(board.post_count, "posts")}
          ${this.renderMetric(board.root_count ?? 0, "roots")}
        </div>
        <div class="intro__extra" data-board-intro-extra>
          <p class="item-card__meta">${meta([
            `Category: ${board.category_name}`,
            `Masters: ${masters}`,
          ])}</p>
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
      ${this.renderUserSection("New users", data.new_users)}
      ${this.renderUserSection("Top point users", data.top_point_users)}
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
    const author = post.user_name || (post.user_id ? `user ${post.user_id}` : null);
    const type = postTypeLabels[post.post_type] || "Post";
    const typeIcon = postTypeIcons[post.post_type] || postTypeIcons[0];
    const typeClass = postTypeClasses[post.post_type] || postTypeClasses[0];
    const statusBar = renderPostStatusBar(post);
    const metadata = [
      post.board_name
        ? this.renderPostMetaItem(postMetaIcons.board, post.board_name, "Board")
        : "",
      author
        ? this.renderPostMetaItem(
            postMetaIcons.author,
            author,
            "Author",
            post.user_id ? `/user/${encodeURIComponent(post.user_id)}` : null,
            false,
            true,
          )
        : "",
      post.post_time ? this.renderPostMetaItem(postMetaIcons.time, post.post_time, "Posted") : "",
      this.renderPostMetaItem(postMetaIcons.replies, post.reply_count ?? 0, "Replies"),
      this.renderPostMetaItem(postMetaIcons.views, post.access_count ?? 0, "Views"),
      this.renderPostMetaItem(postMetaIcons.points, post.point ?? 0, "Points"),
    ];

    return `
      <article class="item-card">
        <span class="item-card__icon item-card__icon--post ${typeClass}" title="${escapeHtml(type)}">${typeIcon}</span>
        <div class="item-card__content">
          <div class="item-card__title-row">
            <a class="item-card__title" href="/post/${post.id}" target="_blank" rel="noopener">${postTitle(post)}</a>
            ${statusBar}
          </div>
          <p class="item-card__meta post-meta">${metadata.join("")}</p>
        </div>
      </article>
    `;
  }

  renderUserSection(title, users) {
    return `
      <section class="section" aria-labelledby="${this.sectionId(title)}">
        <div class="section__header">
          ${sectionIcons.users}
          <h2 id="${this.sectionId(title)}">${escapeHtml(title)}</h2>
        </div>
        <div class="item-list">
          ${
            users.length
              ? users.map((user) => this.renderUserCard(user)).join("")
              : `<p class="section__state">No users.</p>`
          }
        </div>
      </section>
    `;
  }

  renderUserCard(user) {
    const joined = user.reg_time || "date unknown";
    return `
      <article class="item-card item-card--compact user-card">
        <span class="item-card__icon">${userListIcon}</span>
        <div class="user-card__body">
          <a class="item-card__title" href="/user/${encodeURIComponent(user.id)}" target="_blank" rel="noopener">${escapeHtml(user.name)}</a>
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
    const editorPath = `/post_upd?board_id=${encodeURIComponent(data.board.id)}`;
    const addHref = this.session.loggedIn
      ? editorPath
      : `/login?return_to=${encodeURIComponent(editorPath)}`;
    return `
      <nav class="board-actions section section--wide" aria-label="Board operations">
        <a class="post-create-button" href="${addHref}">
          ${postActionIcons.add}
          <span>Add post</span>
        </a>
      </nav>
      ${this.renderPager(data.pager, data.board.id)}
      ${this.renderPostTrees(data.trees, true)}
      ${this.renderPager(data.pager, data.board.id)}
    `;
  }

  renderUserPage(data) {
    return `
      ${this.renderUserStatus(data)}
      <section class="section section--wide activity-panel" aria-label="User activities">
        ${this.renderUserActivityTabs(data)}
        <div class="item-list">
          ${
            data.posts.length
              ? data.posts.map((post) => this.renderPostCard(post)).join("")
              : `<p class="section__state">No posts.</p>`
          }
        </div>
      </section>
      ${this.renderUserPager(data)}
    `;
  }

  renderUserListPage(data) {
    return `
      <section class="section section--wide user-directory" aria-label="User list controls">
        <div class="section__header">
          ${sectionIcons.users}
          <h2>User list</h2>
        </div>
        <form class="user-directory__filters" method="get" action="/user_list">
          <label class="login-field">
            <span>User name or email</span>
            <input type="search" name="query" value="${escapeHtml(data.query || "")}">
          </label>
          <label class="login-field">
            <span>Order</span>
            <select name="order">
              <option value="id_desc"${data.order === "id_desc" ? " selected" : ""}>Newest ID first</option>
              <option value="id_asc"${data.order === "id_asc" ? " selected" : ""}>Oldest ID first</option>
            </select>
          </label>
          <label class="login-field">
            <span>Role</span>
            <select name="role">
              <option value=""${data.role == null && !data.active ? " selected" : ""}>All roles</option>
              <option value="active"${data.active ? " selected" : ""}>Active</option>
              <option value="0"${data.role === 0 ? " selected" : ""}>Frozen</option>
              <option value="1"${data.role === 1 ? " selected" : ""}>Member</option>
              <option value="5"${data.role === 5 ? " selected" : ""}>Advanced</option>
              <option value="10"${data.role === 10 ? " selected" : ""}>Administrator</option>
            </select>
          </label>
          <button class="login-submit" type="submit">Search</button>
        </form>
      </section>
      <section class="section section--wide user-directory__results" aria-label="Users">
        <div class="user-table__scroll">
          <table class="user-table">
            <thead>
              <tr>
                <th scope="col">ID</th>
                <th scope="col">User name</th>
                <th scope="col">Role</th>
                <th scope="col">Email</th>
                <th scope="col">Registered</th>
                <th scope="col">Posts</th>
                <th scope="col">Originals</th>
                <th scope="col">Favorites</th>
                <th scope="col">Points</th>
                <th scope="col">Last login</th>
              </tr>
            </thead>
            <tbody>
              ${
                data.users.length
                  ? data.users.map((user) => this.renderUserTableRow(user)).join("")
                  : `<tr><td class="user-table__empty" colspan="10">No matching users.</td></tr>`
              }
            </tbody>
          </table>
        </div>
      </section>
      ${this.renderUserListPager(data)}
    `;
  }

  renderUserAddPage() {
    return `
      <section class="section section--wide user-add" aria-label="Add user">
        <div class="section__header">
          ${sectionIcons.users}
          <h2>Add user</h2>
        </div>
        <form class="user-add__form" data-user-add-form>
          <div class="user-add__fields">
            <label class="login-field">
              <span>User name</span>
              <input name="name" maxlength="25" autocomplete="off" required>
            </label>
            <label class="login-field">
              <span>Email</span>
              <input type="email" name="email" maxlength="25" autocomplete="off">
            </label>
            <section class="user-add__password-suggestion" aria-label="Suggested password">
              <span>Suggested password</span>
              <output data-suggested-password></output>
              <div class="user-add__password-actions">
                <button class="tool-button" type="button" data-new-password title="Generate new password" aria-label="Generate new password">${userActionIcons.regenerate}</button>
                <button class="tool-button" type="button" data-copy-password title="Copy password" aria-label="Copy password">${userActionIcons.copy}</button>
              </div>
              <p data-copy-password-state hidden aria-live="polite"></p>
            </section>
            <label class="login-field">
              <span>Password</span>
              <input type="password" name="password" autocomplete="new-password" minlength="8" maxlength="30" required>
            </label>
            <label class="login-field">
              <span>Confirm password</span>
              <input type="password" name="confirm_password" autocomplete="new-password" minlength="8" maxlength="30" required>
            </label>
            <label class="login-field user-add__intro">
              <span>Introduction</span>
              <textarea name="intro" maxlength="100" rows="3"></textarea>
            </label>
          </div>
          <section class="user-add__introducer" aria-label="Introducing user">
            <h3>Introduced by</h3>
            <input type="hidden" name="intro_user_id" data-intro-user-id>
            <p class="user-add__selected" data-intro-user-selected>No introducing user selected.</p>
            <div class="user-add__search">
              <label class="login-field">
                <span>Search user name or email</span>
                <input type="search" data-intro-user-query autocomplete="off">
              </label>
              <button class="password-change__cancel" type="button" data-intro-user-search>Search</button>
              <button class="password-change__cancel" type="button" data-intro-user-clear>Clear</button>
            </div>
            <div class="site-master-results user-add__results" data-intro-user-results hidden></div>
          </section>
          <p class="password-change__policy">8 to 30 characters; include a letter, a number, and an ASCII symbol. Spaces and non-ASCII characters are not accepted.</p>
          <p class="login-form__error" data-user-add-error hidden></p>
          <div class="password-change__buttons">
            <button class="login-submit" type="submit">Add user</button>
          </div>
        </form>
      </section>
    `;
  }

  renderSiteManagerPage(data) {
    return `
      <section class="section section--wide site-manager__controller" aria-label="Site manager operations">
        <div class="section__header">
          ${userActionIcons.calculate}
          <h2>Operations</h2>
          <button class="password-change__cancel" type="button" data-create-category-toggle>Add category</button>
          <button class="password-change__cancel" type="button" data-all-board-statistics-toggle>Recalculate board statistics</button>
        </div>
        <form class="statistics-confirmation site-create-form" data-create-category-form hidden>
          <h2>Add category</h2>
          <div class="site-fields site-fields--category">
            <label class="login-field">
              <span>Category name</span>
              <input name="name" required>
            </label>
            <label class="login-field">
              <span>Description</span>
              <input name="comment">
            </label>
            <label class="login-field site-fields__order">
              <span>Order</span>
              <input type="number" name="order_id" value="0" required>
            </label>
            <button class="login-submit" type="submit">Add category</button>
          </div>
          <p class="login-form__error site-manager__message" data-site-error hidden></p>
          <button class="password-change__cancel site-create-form__cancel" type="button" data-create-category-cancel>Cancel</button>
        </form>
        <section class="statistics-confirmation" data-all-board-statistics-confirmation hidden>
          <h2>Recalculate statistics for all boards?</h2>
          <p>This operation updates board post and root counts, category board counts, and board-master derived member roles. Deleted and unsupported-state posts are excluded; administrator and frozen roles are not changed.</p>
          <div class="password-change__buttons">
            <button class="login-submit" type="button" data-all-board-statistics-confirm>Recalculate</button>
            <button class="password-change__cancel" type="button" data-all-board-statistics-cancel>Cancel</button>
          </div>
        </section>
        <p class="login-form__error site-manager__message" data-site-error hidden></p>
      </section>
      <section class="section section--wide site-manager__intro" aria-label="Site manager">
        <div class="section__header">
          ${sectionIcons.boards}
          <h2>Boards and categories</h2>
        </div>
      </section>
      ${data.categories
        .map((category) => {
          const boards = data.boards.filter((board) => board.category_id === category.id);
          return this.renderSiteCategoryEditor(category, boards, data.categories);
        })
        .join("")}
    `;
  }

  renderSiteCategoryEditor(category, boards, categories) {
    return `
      <section class="section section--wide site-category-editor" aria-labelledby="site-category-${escapeHtml(category.id)}">
        <form class="site-category-form" data-category-form data-category-id="${escapeHtml(category.id)}">
          <div class="site-category-form__heading">
            ${sectionIcons.boards}
            <h2 id="site-category-${escapeHtml(category.id)}">${escapeHtml(category.name)}</h2>
            <span class="badge">${escapeHtml(category.board_count)} boards</span>
            <div class="site-category-form__actions">
              <button class="password-change__cancel" type="button" data-create-board-toggle>Add board</button>
              <button class="password-change__cancel" type="button" data-delete-category-toggle>Delete category</button>
            </div>
          </div>
          <div class="site-fields site-fields--category">
            <label class="login-field">
              <span>Category name</span>
              <input name="name" value="${escapeHtml(category.name)}" required>
            </label>
            <label class="login-field">
              <span>Description</span>
              <input name="comment" value="${escapeHtml(category.comment || "")}">
            </label>
            <label class="login-field site-fields__order">
              <span>Order</span>
              <input type="number" name="order_id" value="${escapeHtml(category.order_id)}" required>
            </label>
            <button class="login-submit" type="submit">Save category</button>
          </div>
          <section class="statistics-confirmation" data-delete-category-confirmation hidden>
            <h2>Delete category ${escapeHtml(category.name)}?</h2>
            <p>This succeeds only if the category contains no boards.</p>
            <div class="password-change__buttons">
              <button class="login-submit" type="button" data-delete-category-confirm>Delete</button>
              <button class="password-change__cancel" type="button" data-delete-category-cancel>Cancel</button>
            </div>
          </section>
          <p class="login-form__error site-manager__message" data-site-error hidden></p>
        </form>
        <form class="statistics-confirmation site-create-form" data-create-board-form data-category-id="${escapeHtml(category.id)}" hidden>
          <h2>Add board to ${escapeHtml(category.name)}</h2>
          <div class="site-fields site-fields--board-create">
            <label class="login-field">
              <span>Board name</span>
              <input name="name" required>
            </label>
            <label class="login-field site-fields__comment">
              <span>Description</span>
              <input name="comment">
            </label>
            <label class="login-field site-fields__order">
              <span>Order</span>
              <input type="number" name="order_id" value="0" required>
            </label>
            <button class="login-submit" type="submit">Add board</button>
          </div>
          <p class="login-form__error site-manager__message" data-site-error hidden></p>
          <button class="password-change__cancel site-create-form__cancel" type="button" data-create-board-cancel>Cancel</button>
        </form>
        <div class="site-board-list">
          ${
            boards.length
              ? boards.map((board) => this.renderSiteBoardEditor(board, categories)).join("")
              : `<p class="section__state">No boards in this category.</p>`
          }
        </div>
      </section>
    `;
  }

  renderSiteBoardEditor(board, categories) {
    return `
      <form class="site-board-form" data-board-form data-board-id="${escapeHtml(board.id)}">
        <header class="site-board-form__header">
          <a class="item-card__title site-board-form__link" href="/board/${encodeURIComponent(board.id)}">${escapeHtml(board.name)}</a>
          <div class="site-board-form__metrics">
            ${this.renderMetric(board.post_count ?? 0, "posts")}
            ${this.renderMetric(board.root_count ?? 0, "roots")}
          </div>
          <button class="password-change__cancel" type="button" data-delete-board-toggle>Delete board</button>
        </header>
        <section class="statistics-confirmation" data-delete-board-confirmation hidden>
          <h2>Delete board ${escapeHtml(board.name)}?</h2>
          <p>This succeeds only if the board contains no posts.</p>
          <div class="password-change__buttons">
            <button class="login-submit" type="button" data-delete-board-confirm>Delete</button>
            <button class="password-change__cancel" type="button" data-delete-board-cancel>Cancel</button>
          </div>
        </section>
        <div class="site-fields site-fields--board">
          <label class="login-field">
            <span>Board name</span>
            <input name="name" value="${escapeHtml(board.name)}" required>
          </label>
          <label class="login-field site-fields__comment">
            <span>Description</span>
            <input name="comment" value="${escapeHtml(board.comment || "")}">
          </label>
          <label class="login-field">
            <span>Category</span>
            <select name="category_id">
              ${categories
                .map(
                  (category) =>
                    `<option value="${escapeHtml(category.id)}"${category.id === board.category_id ? " selected" : ""}>${escapeHtml(category.name)}</option>`,
                )
                .join("")}
            </select>
          </label>
          <label class="login-field site-fields__order">
            <span>Order</span>
            <input type="number" name="order_id" value="${escapeHtml(board.order_id)}" required>
          </label>
          <button class="login-submit" type="submit">Save board</button>
        </div>
        <div class="site-master-editor">
          <div class="site-master-fields" data-master-fields>
            ${
              board.masters.length
                ? board.masters.map((master) => this.renderSiteMasterRow(master)).join("")
                : `<p class="site-master-fields__empty" data-master-empty>No board masters assigned.</p>`
            }
          </div>
          <button class="password-change__cancel" type="button" data-add-master>Add master</button>
        </div>
        <section class="site-master-picker" data-master-picker hidden>
          <h2>Add board master</h2>
          <div class="site-master-picker__controls">
            <label class="login-field">
              <span>User name</span>
              <input type="search" data-master-search autocomplete="off">
            </label>
            <button class="login-submit" type="button" data-master-search-button>Search</button>
            <button class="password-change__cancel" type="button" data-master-picker-cancel>Cancel</button>
          </div>
          <div class="site-master-results" data-master-results>
            <p class="section__state">Search for a user to assign as board master.</p>
          </div>
        </section>
        <p class="login-form__error site-manager__message" data-site-error hidden></p>
      </form>
    `;
  }

  renderSiteMasterRow(master) {
    return `
      <div class="site-master-field" data-master-row>
        <input type="hidden" name="master_user_id" value="${escapeHtml(master.id)}">
        <a href="/user/${encodeURIComponent(master.id)}" target="_blank" rel="noopener">${escapeHtml(master.name)}</a>
        <button class="password-change__cancel site-master-field__remove" type="button" data-remove-master>Remove</button>
      </div>
    `;
  }

  renderSiteMasterResults(users, selectedIds) {
    const choices = users.filter((user) => !selectedIds.has(Number(user.id)));
    if (!choices.length) {
      return `<p class="section__state">No unassigned matching users.</p>`;
    }
    return choices
      .map(
        (user) => `
          <button class="site-master-result" type="button" data-select-master data-user-id="${escapeHtml(user.id)}" data-user-name="${escapeHtml(user.name)}">
            <span>${escapeHtml(user.name)}</span>
            <span class="site-master-result__meta">ID ${escapeHtml(user.id)}</span>
          </button>
        `,
      )
      .join("");
  }

  bindSiteManagerActions() {
    const controller = this.querySelector(".site-manager__controller");
    const createCategoryToggle = controller.querySelector("[data-create-category-toggle]");
    const createCategoryForm = controller.querySelector("[data-create-category-form]");
    createCategoryToggle.addEventListener("click", () => {
      createCategoryForm.hidden = false;
      createCategoryForm.querySelector("[name='name']").focus();
    });
    createCategoryForm.querySelector("[data-create-category-cancel]").addEventListener("click", () => {
      createCategoryForm.hidden = true;
      createCategoryToggle.focus();
    });
    createCategoryForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const fields = new FormData(createCategoryForm);
      await this.submitSiteManagerForm(createCategoryForm, () =>
        submitCategoryCreation({
          name: String(fields.get("name") || ""),
          comment: String(fields.get("comment") || ""),
          order_id: Number(fields.get("order_id") || 0),
        }),
      );
    });
    const toggleAllStatistics = controller.querySelector("[data-all-board-statistics-toggle]");
    const allStatisticsConfirmation = controller.querySelector("[data-all-board-statistics-confirmation]");
    toggleAllStatistics.addEventListener("click", () => {
      allStatisticsConfirmation.hidden = false;
    });
    allStatisticsConfirmation.querySelector("[data-all-board-statistics-cancel]").addEventListener("click", () => {
      allStatisticsConfirmation.hidden = true;
      toggleAllStatistics.focus();
    });
    allStatisticsConfirmation.querySelector("[data-all-board-statistics-confirm]").addEventListener("click", async () => {
      await this.submitSiteManagerForm(controller, () => submitBoardStatisticsRecalculation());
    });
    this.querySelectorAll("[data-category-form]").forEach((form) => {
      form.addEventListener("submit", async (event) => {
        event.preventDefault();
        const fields = new FormData(form);
        await this.submitSiteManagerForm(form, () =>
          submitCategoryUpdate(form.dataset.categoryId, {
            name: String(fields.get("name") || ""),
            comment: String(fields.get("comment") || ""),
            order_id: Number(fields.get("order_id") || 0),
          }),
        );
      });
      const deleteToggle = form.querySelector("[data-delete-category-toggle]");
      const deleteConfirmation = form.querySelector("[data-delete-category-confirmation]");
      deleteToggle.addEventListener("click", () => {
        deleteConfirmation.hidden = false;
      });
      deleteConfirmation.querySelector("[data-delete-category-cancel]").addEventListener("click", () => {
        deleteConfirmation.hidden = true;
        deleteToggle.focus();
      });
      deleteConfirmation.querySelector("[data-delete-category-confirm]").addEventListener("click", async () => {
        await this.submitSiteManagerForm(form, () => submitCategoryDeletion(form.dataset.categoryId));
      });
    });
    this.querySelectorAll("[data-create-board-form]").forEach((form) => {
      const section = form.closest(".site-category-editor");
      const toggle = section.querySelector("[data-create-board-toggle]");
      toggle.addEventListener("click", () => {
        form.hidden = false;
        form.querySelector("[name='name']").focus();
      });
      form.querySelector("[data-create-board-cancel]").addEventListener("click", () => {
        form.hidden = true;
        toggle.focus();
      });
      form.addEventListener("submit", async (event) => {
        event.preventDefault();
        const fields = new FormData(form);
        await this.submitSiteManagerForm(form, () =>
          submitBoardCreation({
            name: String(fields.get("name") || ""),
            comment: String(fields.get("comment") || ""),
            category_id: Number(form.dataset.categoryId),
            order_id: Number(fields.get("order_id") || 0),
          }),
        );
      });
    });
    this.querySelectorAll("[data-board-form]").forEach((form) => {
      form.addEventListener("submit", async (event) => {
        event.preventDefault();
        const fields = new FormData(form);
        await this.submitSiteManagerForm(form, () =>
          submitBoardUpdate(form.dataset.boardId, {
            name: String(fields.get("name") || ""),
            comment: String(fields.get("comment") || ""),
            category_id: Number(fields.get("category_id") || 0),
            order_id: Number(fields.get("order_id") || 0),
          }),
        );
      });
      const deleteToggle = form.querySelector("[data-delete-board-toggle]");
      const deleteConfirmation = form.querySelector("[data-delete-board-confirmation]");
      deleteToggle.addEventListener("click", () => {
        deleteConfirmation.hidden = false;
        deleteConfirmation.scrollIntoView({ block: "nearest" });
        deleteConfirmation.querySelector("[data-delete-board-confirm]").focus();
      });
      deleteConfirmation.querySelector("[data-delete-board-cancel]").addEventListener("click", () => {
        deleteConfirmation.hidden = true;
        deleteToggle.focus();
      });
      deleteConfirmation.querySelector("[data-delete-board-confirm]").addEventListener("click", async () => {
        await this.submitSiteManagerForm(form, () => submitBoardDeletion(form.dataset.boardId));
      });
      form.querySelector("[data-add-master]").addEventListener("click", () => {
        const picker = form.querySelector("[data-master-picker]");
        picker.hidden = false;
        picker.querySelector("[data-master-search]").focus();
      });
      const masterFields = form.querySelector("[data-master-fields]");
      const picker = form.querySelector("[data-master-picker]");
      const searchInput = picker.querySelector("[data-master-search]");
      const results = picker.querySelector("[data-master-results]");
      const performMasterSearch = async () => {
        const query = searchInput.value.trim();
        if (!query) {
          results.innerHTML = `<p class="section__state">Enter a user name to search.</p>`;
          return;
        }
        results.innerHTML = `<p class="section__state">Searching users...</p>`;
        try {
          const response = await getUserList(query, "", "id_asc", 1);
          const selectedIds = new Set(
            [...masterFields.querySelectorAll("[name='master_user_id']")].map((input) => Number(input.value)),
          );
          results.innerHTML = this.renderSiteMasterResults(response.users, selectedIds);
        } catch (requestError) {
          results.innerHTML = `<p class="section__state">Unable to search users.</p>`;
          console.error(requestError);
        }
      };
      picker.querySelector("[data-master-search-button]").addEventListener("click", performMasterSearch);
      searchInput.addEventListener("keydown", (event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          performMasterSearch();
        }
      });
      picker.querySelector("[data-master-picker-cancel]").addEventListener("click", () => {
        picker.hidden = true;
      });
      results.addEventListener("click", async (event) => {
        const select = event.target.closest("[data-select-master]");
        if (!select) {
          return;
        }
        const userId = Number(select.dataset.userId);
        select.disabled = true;
        await this.submitSiteMasterMutation(form, () => submitBoardMasterAddition(form.dataset.boardId, userId), (response) => {
          masterFields.querySelector("[data-master-empty]")?.remove();
          masterFields.insertAdjacentHTML("beforeend", this.renderSiteMasterRow(response.master));
          picker.hidden = true;
        });
        if (select.isConnected) {
          select.disabled = false;
        }
      });
      masterFields.addEventListener("click", async (event) => {
        const remove = event.target.closest("[data-remove-master]");
        if (!remove) {
          return;
        }
        const row = remove.closest("[data-master-row]");
        const userId = Number(row.querySelector("[name='master_user_id']").value);
        remove.disabled = true;
        await this.submitSiteMasterMutation(form, () => submitBoardMasterRemoval(form.dataset.boardId, userId), () => {
          row.remove();
          if (!masterFields.querySelector("[data-master-row]")) {
            masterFields.innerHTML = `<p class="site-master-fields__empty" data-master-empty>No board masters assigned.</p>`;
          }
        });
        if (remove.isConnected) {
          remove.disabled = false;
        }
      });
    });
  }

  async submitSiteManagerForm(form, submit) {
    const error = form.querySelector("[data-site-error]");
    const controls = form.querySelectorAll("button");
    error.hidden = true;
    controls.forEach((control) => {
      control.disabled = true;
    });
    try {
      await submit();
      await this.loadSiteManager();
    } catch (requestError) {
      error.textContent = requestError.message || "Unable to save site management changes.";
      error.hidden = false;
      controls.forEach((control) => {
        control.disabled = false;
      });
    }
  }

  async submitSiteMasterMutation(form, submit, applyResult) {
    const error = form.querySelector("[data-site-error]");
    error.hidden = true;
    try {
      const result = await submit();
      applyResult(result);
    } catch (requestError) {
      error.textContent = requestError.message || "Unable to update board masters.";
      error.hidden = false;
    }
  }

  renderUserTableRow(user) {
    return `
      <tr>
        <td>${escapeHtml(user.id)}</td>
        <td><a href="/user/${encodeURIComponent(user.id)}" target="_blank" rel="noopener">${escapeHtml(user.name)}</a></td>
        <td>${escapeHtml(this.userLevelLabel(user.level))}</td>
        <td>${escapeHtml(user.email || "")}</td>
        <td>${escapeHtml(user.reg_time || "")}</td>
        <td>${escapeHtml(user.post_count ?? 0)}</td>
        <td>${escapeHtml(user.doc_count ?? 0)}</td>
        <td>${escapeHtml(user.favorite_count ?? 0)}</td>
        <td>${escapeHtml(user.point ?? 0)}</td>
        <td>${escapeHtml(user.last_login || "")}</td>
      </tr>
    `;
  }

  renderUserStatus(data) {
    const user = data.user;
    const joined = user.reg_time || "date unknown";
    const requiresCurrentPassword = Number(this.session.user?.level || 0) < 10;
    return `
      <section class="user-profile section section--wide" aria-label="User status">
        <div class="user-profile__main">
          <span class="user-profile__icon">${userListIcon}</span>
          <div class="user-profile__body">
            <div class="user-profile__heading">
              <h1>${escapeHtml(user.name)}</h1>
              <span class="badge">${escapeHtml(this.userLevelLabel(user.level))}</span>
            </div>
            <p class="post-meta item-card__meta">
              ${this.renderPostMetaItem(postMetaIcons.time, joined, "Joined", null, true)}
              ${
                user.last_login
                  ? this.renderPostMetaItem(postMetaIcons.replyTime, user.last_login, "Last login", null, true)
                  : ""
              }
              ${this.renderUserPrivateDetails(data.private_details)}
            </p>
            ${this.renderUserIntro(user.intro)}
          </div>
          <div class="user-profile__metrics" aria-label="User statistics">
            ${this.renderMetric(user.post_count ?? 0, "posts")}
            ${this.renderMetric(user.doc_count ?? 0, "originals")}
            ${this.renderMetric(user.point ?? 0, "points")}
          </div>
        </div>
        ${this.renderUserSignature(data.latest_signature)}
        ${
          data.can_update
            ? `
              <div class="user-profile__actions" aria-label="Profile operations">
                <button class="tool-button" type="button" title="Change password" aria-label="Change password" data-change-password-toggle aria-expanded="false">
                  ${userActionIcons.password}
                </button>
                <button class="tool-button" type="button" title="Recalculate statistics" aria-label="Recalculate statistics" data-recalculate-statistics>
                  ${userActionIcons.calculate}
                </button>
                <button class="tool-button" type="button" title="Update profile" aria-label="Update profile" data-profile-update-toggle aria-expanded="false">
                  ${userActionIcons.update}
                </button>
                ${
                  data.can_set_role
                    ? `
                      <button class="tool-button" type="button" title="Set role" aria-label="Set role" data-set-role-toggle aria-expanded="false">
                        ${userActionIcons.role}
                      </button>
                    `
                    : ""
                }
              </div>
              ${this.renderStatisticsConfirmation(user)}
              ${this.renderProfileUpdateForm(data)}
              ${data.can_set_role ? this.renderRoleChangeForm(user) : ""}
              ${this.renderPasswordChangeForm(user, requiresCurrentPassword)}
            `
            : ""
        }
      </section>
    `;
  }

  renderPasswordChangeForm(user, requiresCurrentPassword) {
    return `
      <form class="password-change" data-password-change-form data-user-id="${escapeHtml(user.id)}" hidden>
        <h2>Change password for ${escapeHtml(user.name)}</h2>
        ${
          requiresCurrentPassword
            ? `
              <label class="login-field">
                <span>Current password</span>
                <input type="password" name="current_password" autocomplete="current-password" required>
              </label>
            `
            : `<p class="password-change__notice">Administrator reset: the current password is not required.</p>`
        }
        <label class="login-field">
          <span>New password</span>
          <input type="password" name="new_password" autocomplete="new-password" minlength="8" maxlength="30" required>
        </label>
        <label class="login-field">
          <span>Confirm new password</span>
          <input type="password" name="confirm_password" autocomplete="new-password" minlength="8" maxlength="30" required>
        </label>
        <p class="password-change__policy">8 to 30 characters; include a letter, a number, and an ASCII symbol. Spaces and non-ASCII characters are not accepted.</p>
        <p class="login-form__error" data-password-change-error hidden></p>
        <p class="password-change__success" data-password-change-success hidden>Password changed.</p>
        <div class="password-change__buttons">
          <button class="login-submit" type="submit">Change password</button>
          <button class="password-change__cancel" type="button" data-password-change-cancel>Cancel</button>
        </div>
      </form>
    `;
  }

  renderStatisticsConfirmation(user) {
    return `
      <section class="statistics-confirmation" aria-label="Recalculate statistics" data-statistics-confirmation hidden>
        <h2>Recalculate statistics for ${escapeHtml(user.name)}?</h2>
        <p>This operation updates the stored post, original-post, and favorite counts from currently visible forum records. Deleted and unsupported-state posts are excluded.</p>
        <p class="login-form__error" data-statistics-error hidden></p>
        <div class="password-change__buttons">
          <button class="login-submit" type="button" data-statistics-confirm>Recalculate</button>
          <button class="password-change__cancel" type="button" data-statistics-cancel>Cancel</button>
        </div>
      </section>
    `;
  }

  renderRoleChangeForm(user) {
    const selectedLevel = Number(user.level) === 10 ? 10 : Number(user.level) === 0 ? 0 : 1;
    return `
      <form class="statistics-confirmation role-change" data-role-change-form hidden>
        <h2>Set role for ${escapeHtml(user.name)}</h2>
        <p>Advanced is assigned automatically while a member is a board master. Administrator and frozen roles are not changed by board-master assignment.</p>
        <label class="login-field">
          <span>Role</span>
          <select name="level">
            <option value="0"${selectedLevel === 0 ? " selected" : ""}>Frozen</option>
            <option value="1"${selectedLevel === 1 ? " selected" : ""}>Member</option>
            <option value="10"${selectedLevel === 10 ? " selected" : ""}>Administrator</option>
          </select>
        </label>
        <p class="login-form__error" data-role-change-error hidden></p>
        <div class="password-change__buttons">
          <button class="login-submit" type="submit">Set role</button>
          <button class="password-change__cancel" type="button" data-role-change-cancel>Cancel</button>
        </div>
      </form>
    `;
  }

  renderProfileUpdateForm(data) {
    return `
      <form class="statistics-confirmation profile-update" data-profile-update-form hidden>
        <h2>Update profile for ${escapeHtml(data.user.name)}</h2>
        <label class="login-field">
          <span>Email</span>
          <input type="email" name="email" maxlength="25" value="${escapeHtml(data.private_details?.email || "")}">
        </label>
        <label class="login-field">
          <span>Introduction</span>
          <textarea name="intro" maxlength="100" rows="3">${escapeHtml(data.user.intro || "")}</textarea>
        </label>
        <p class="login-form__error" data-profile-update-error hidden></p>
        <div class="password-change__buttons">
          <button class="login-submit" type="submit">Update</button>
          <button class="password-change__cancel" type="button" data-profile-update-cancel>Cancel</button>
        </div>
      </form>
    `;
  }

  bindUserActions(data) {
    const toggle = this.querySelector("[data-change-password-toggle]");
    const form = this.querySelector("[data-password-change-form]");
    const recalculate = this.querySelector("[data-recalculate-statistics]");
    const confirmation = this.querySelector("[data-statistics-confirmation]");
    const statisticsError = this.querySelector("[data-statistics-error]");
    const statisticsConfirm = this.querySelector("[data-statistics-confirm]");
    const roleToggle = this.querySelector("[data-set-role-toggle]");
    const roleForm = this.querySelector("[data-role-change-form]");
    const profileToggle = this.querySelector("[data-profile-update-toggle]");
    const profileForm = this.querySelector("[data-profile-update-form]");
    if (!toggle || !form) {
      return;
    }
    const error = form.querySelector("[data-password-change-error]");
    const success = form.querySelector("[data-password-change-success]");
    const setOpen = (open) => {
      form.hidden = !open;
      toggle.setAttribute("aria-expanded", String(open));
      if (open) {
        form.querySelector("input")?.focus();
      }
    };
    toggle.addEventListener("click", () => setOpen(form.hidden));
    form.querySelector("[data-password-change-cancel]")?.addEventListener("click", () => {
      form.reset();
      error.hidden = true;
      success.hidden = true;
      setOpen(false);
    });
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      const button = form.querySelector("button[type=submit]");
      const fields = new FormData(form);
      error.hidden = true;
      success.hidden = true;
      button.disabled = true;
      try {
        const result = await submitPasswordChange(
          data.user.id,
          String(fields.get("current_password") || ""),
          String(fields.get("new_password") || ""),
          String(fields.get("confirm_password") || ""),
        );
        form.reset();
        if (result.session_invalidated) {
          window.location.assign(`/login?return_to=${encodeURIComponent(localPagePath())}`);
          return;
        }
        success.hidden = false;
      } catch (requestError) {
        error.textContent = requestError.message || "Unable to change password.";
        error.hidden = false;
      } finally {
        button.disabled = false;
      }
    });
    recalculate?.addEventListener("click", () => {
      confirmation.hidden = false;
      statisticsError.hidden = true;
      statisticsConfirm.focus();
    });
    confirmation?.querySelector("[data-statistics-cancel]")?.addEventListener("click", () => {
      confirmation.hidden = true;
      statisticsError.hidden = true;
      recalculate.focus();
    });
    statisticsConfirm?.addEventListener("click", async () => {
      recalculate.disabled = true;
      statisticsConfirm.disabled = true;
      statisticsError.hidden = true;
      try {
        await submitStatisticsRecalculation(data.user.id);
        await this.loadUser(data.user.id, this.currentActivity(), this.currentPage());
      } catch (requestError) {
        statisticsError.textContent = requestError.message || "Unable to recalculate statistics.";
        statisticsError.hidden = false;
      } finally {
        recalculate.disabled = false;
        statisticsConfirm.disabled = false;
      }
    });
    profileToggle?.addEventListener("click", () => {
      profileForm.hidden = false;
      profileToggle.setAttribute("aria-expanded", "true");
      profileForm.querySelector("input").focus();
    });
    profileForm?.querySelector("[data-profile-update-cancel]")?.addEventListener("click", () => {
      profileForm.hidden = true;
      profileToggle.setAttribute("aria-expanded", "false");
      profileToggle.focus();
    });
    profileForm?.addEventListener("submit", async (event) => {
      event.preventDefault();
      const error = profileForm.querySelector("[data-profile-update-error]");
      const submit = profileForm.querySelector("button[type=submit]");
      const fields = new FormData(profileForm);
      error.hidden = true;
      submit.disabled = true;
      try {
        await submitProfileUpdate(
          data.user.id,
          String(fields.get("email") || ""),
          String(fields.get("intro") || ""),
        );
        await this.loadUser(data.user.id, this.currentActivity(), this.currentPage());
      } catch (requestError) {
        error.textContent = requestError.message || "Unable to update profile.";
        error.hidden = false;
        submit.disabled = false;
      }
    });
    roleToggle?.addEventListener("click", () => {
      roleForm.hidden = false;
      roleToggle.setAttribute("aria-expanded", "true");
      roleForm.querySelector("select").focus();
    });
    roleForm?.querySelector("[data-role-change-cancel]")?.addEventListener("click", () => {
      roleForm.hidden = true;
      roleToggle.setAttribute("aria-expanded", "false");
      roleToggle.focus();
    });
    roleForm?.addEventListener("submit", async (event) => {
      event.preventDefault();
      const error = roleForm.querySelector("[data-role-change-error]");
      const submit = roleForm.querySelector("button[type=submit]");
      const fields = new FormData(roleForm);
      error.hidden = true;
      submit.disabled = true;
      try {
        await submitRoleChange(data.user.id, Number(fields.get("level")));
        window.location.reload();
      } catch (requestError) {
        error.textContent = requestError.message || "Unable to set role.";
        error.hidden = false;
        submit.disabled = false;
      }
    });
  }

  bindUserAddActions() {
    const form = this.querySelector("[data-user-add-form]");
    if (!form) {
      return;
    }
    const error = form.querySelector("[data-user-add-error]");
    const button = form.querySelector("button[type=submit]");
    const introUserId = form.querySelector("[data-intro-user-id]");
    const introSelected = form.querySelector("[data-intro-user-selected]");
    const introQuery = form.querySelector("[data-intro-user-query]");
    const introResults = form.querySelector("[data-intro-user-results]");
    const suggestedPassword = form.querySelector("[data-suggested-password]");
    const copyPasswordState = form.querySelector("[data-copy-password-state]");
    const refreshSuggestedPassword = () => {
      suggestedPassword.textContent = generateSuggestedPassword();
      copyPasswordState.hidden = true;
    };
    refreshSuggestedPassword();
    form.querySelector("[data-new-password]")?.addEventListener("click", refreshSuggestedPassword);
    form.querySelector("[data-copy-password]")?.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(suggestedPassword.textContent);
        copyPasswordState.textContent = "Copied.";
      } catch (_copyError) {
        copyPasswordState.textContent = "Unable to copy automatically. Select the displayed password to copy it.";
      }
      copyPasswordState.hidden = false;
    });
    const selectIntroducer = (user) => {
      introUserId.value = String(user.id);
      introSelected.innerHTML = `Selected: <a href="/user/${encodeURIComponent(user.id)}" target="_blank" rel="noopener">${escapeHtml(user.name)}</a>`;
      introSelected.classList.add("is-selected");
      introResults.hidden = true;
    };
    form.querySelector("[data-intro-user-clear]")?.addEventListener("click", () => {
      introUserId.value = "";
      introSelected.textContent = "No introducing user selected.";
      introSelected.classList.remove("is-selected");
      introResults.hidden = true;
      introQuery.value = "";
    });
    const searchIntroducers = async () => {
      const query = introQuery.value.trim();
      error.hidden = true;
      if (!query) {
        introResults.innerHTML = `<p class="section__state">Enter a user name or email.</p>`;
        introResults.hidden = false;
        return;
      }
      try {
        const data = await getUserList(query, "", "id_asc", 1);
        introResults.innerHTML = data.users.length
          ? data.users
              .map(
                (user) => `
                  <button class="site-master-result" type="button" data-intro-user-result="${escapeHtml(user.id)}">
                    <span>${escapeHtml(user.name)}</span>
                    <span class="site-master-result__meta">${escapeHtml(user.email || "")}</span>
                  </button>
                `,
              )
              .join("")
          : `<p class="section__state">No matching users.</p>`;
        introResults.hidden = false;
        introResults.onclick = (event) => {
          const result = event.target.closest("[data-intro-user-result]");
          if (!result) {
            return;
          }
          const user = data.users.find((candidate) => String(candidate.id) === result.dataset.introUserResult);
          if (user) {
            selectIntroducer(user);
          }
        };
      } catch (requestError) {
        error.textContent = requestError.message || "Unable to search users.";
        error.hidden = false;
      }
    };
    form.querySelector("[data-intro-user-search]")?.addEventListener("click", searchIntroducers);
    introQuery?.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        searchIntroducers();
      }
    });
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      const fields = new FormData(form);
      error.hidden = true;
      button.disabled = true;
      try {
        const result = await submitUserCreation({
          name: String(fields.get("name") || ""),
          email: String(fields.get("email") || ""),
          intro: String(fields.get("intro") || ""),
          intro_user_id: fields.get("intro_user_id") ? Number(fields.get("intro_user_id")) : null,
          password: String(fields.get("password") || ""),
          confirm_password: String(fields.get("confirm_password") || ""),
        });
        window.location.assign(`/user/${encodeURIComponent(result.user_id)}`);
      } catch (requestError) {
        error.textContent = requestError.message || "Unable to add user.";
        error.hidden = false;
      } finally {
        button.disabled = false;
      }
    });
  }

  renderUserPrivateDetails(details) {
    if (!details) {
      return "";
    }

    return [
      details.last_login_ip
        ? this.renderPostMetaItem(postMetaIcons.network, details.last_login_ip, "Last login IP", null, true)
        : "",
      details.email
        ? this.renderPostMetaItem(postMetaIcons.email, details.email, "Email", null, true)
        : "",
      details.intro_user_name
        ? this.renderPostMetaItem(
            postMetaIcons.author,
            details.intro_user_name,
            "Introduced by",
            details.intro_user_id ? `/user/${encodeURIComponent(details.intro_user_id)}` : null,
            true,
            true,
          )
        : "",
      details.login_count == null
        ? ""
        : this.renderPostMetaItem(postMetaIcons.views, details.login_count, "Logins", null, true),
    ].join("");
  }

  renderUserIntro(intro) {
    if (!intro) {
      return "";
    }

    return `
      <section class="user-profile__text" aria-label="Introduction">
        <h2>Introduction</h2>
        <p>${escapeHtml(intro)}</p>
      </section>
    `;
  }

  renderUserSignature(signature) {
    if (!signature?.content) {
      return "";
    }

    return `
      <section class="user-profile__signature" aria-label="Latest signature">
        <h2>Latest signature</h2>
        <p>${escapeHtml(signature.content)}</p>
      </section>
    `;
  }

  userLevelLabel(level) {
    return {
      0: "Frozen",
      1: "Member",
      5: "Advanced",
      10: "Administrator",
    }[Number(level)] || "Member";
  }

  renderUserActivityTabs(data) {
    const tabs = [
      ["original", "Original posts"],
      ["favorites", "Favorite posts"],
      ["signatures", "Signature posts"],
    ];
    return `
      <nav class="activity-tabs" aria-label="Activities">
        ${tabs
          .map(([activity, label]) => {
            const selected = data.activity === activity;
            return `
              <a class="activity-tabs__tab${selected ? " is-selected" : ""}" href="${this.userPageHref(data.user.id, activity, 1)}" aria-current="${selected ? "page" : "false"}">${escapeHtml(label)}</a>
            `;
          })
          .join("")}
      </nav>
    `;
  }

  renderUserPager(data) {
    const page = Number(data.pager.page || 1);
    const totalPages = Number(data.pager.total_pages || 0);
    const href = (targetPage) => this.userPageHref(data.user.id, data.activity, targetPage);
    return `
      <nav class="pager section section--wide" aria-label="Activity pagination">
        <a class="pager__button ${page <= 1 ? "is-disabled" : ""}" href="${href(1)}" aria-disabled="${page <= 1}">First</a>
        <a class="pager__button ${page <= 1 ? "is-disabled" : ""}" href="${href(Math.max(1, page - 1))}" aria-disabled="${page <= 1}">Previous</a>
        <span class="pager__status">Page ${escapeHtml(page)} / ${escapeHtml(totalPages || 1)}</span>
        <a class="pager__button ${page >= totalPages ? "is-disabled" : ""}" href="${href(Math.min(totalPages || 1, page + 1))}" aria-disabled="${page >= totalPages}">Next</a>
        <a class="pager__button ${page >= totalPages ? "is-disabled" : ""}" href="${href(totalPages || 1)}" aria-disabled="${page >= totalPages}">Last</a>
      </nav>
    `;
  }

  userPageHref(userId, activity, page) {
    return `/user/${encodeURIComponent(userId)}?activity=${encodeURIComponent(activity)}&page=${encodeURIComponent(page)}`;
  }

  renderUserListPager(data) {
    const page = Number(data.pager.page || 1);
    const totalPages = Number(data.pager.total_pages || 0);
    const role = data.active ? "active" : data.role;
    const href = (targetPage) => this.userListPageHref(data.query, role, data.order, targetPage);
    return `
      <nav class="pager section section--wide" aria-label="User list pagination">
        <a class="pager__button ${page <= 1 ? "is-disabled" : ""}" href="${href(1)}" aria-disabled="${page <= 1}">First</a>
        <a class="pager__button ${page <= 1 ? "is-disabled" : ""}" href="${href(Math.max(1, page - 1))}" aria-disabled="${page <= 1}">Previous</a>
        <span class="pager__status">Page ${escapeHtml(page)} / ${escapeHtml(totalPages || 1)} (${escapeHtml(data.pager.total_users || 0)} users)</span>
        <a class="pager__button ${page >= totalPages ? "is-disabled" : ""}" href="${href(Math.min(totalPages || 1, page + 1))}" aria-disabled="${page >= totalPages}">Next</a>
        <a class="pager__button ${page >= totalPages ? "is-disabled" : ""}" href="${href(totalPages || 1)}" aria-disabled="${page >= totalPages}">Last</a>
      </nav>
    `;
  }

  userListPageHref(query, role, order, page) {
    const params = new URLSearchParams({ query: query || "", role: role == null ? "" : String(role), order, page: String(page) });
    return `/user_list?${params.toString()}`;
  }

  renderPostPage(data) {
    return `
      ${this.renderPostController(data, data.post.id)}
      ${this.renderPostDetail(data.post)}
      ${this.renderPostTree(data.tree, data.post.id)}
    `;
  }

  renderPostEditor(data) {
    const post = data.post || {};
    const isCreate = data.mode === "create";
    const isReply = data.mode === "reply";
    const isUpdate = data.mode === "update";
    const showType = this.postEditorShowsType(data);
    const parent = data.parent || {};
    const editorHeading = isReply
      ? `Reply to: ${parent.subject || "(untitled)"}`
      : isCreate
        ? "Add post"
        : "Update post";
    return `
      <nav class="post-controller section section--wide" aria-label="Post editor navigation">
        <a class="post-controller__board" href="/board/${encodeURIComponent(data.board.id)}">
          ${boardListIcon}
          <span>${escapeHtml(data.board.name)}</span>
        </a>
      </nav>
      <section class="post-editor section section--wide" aria-label="${escapeHtml(editorHeading)}">
        <div class="section__header">
          ${sectionIcons.posts}
          <h2>${escapeHtml(editorHeading)}</h2>
        </div>
        <form class="post-editor__form${isUpdate ? " post-editor__form--update" : ""}" data-post-editor-form>
          <label class="login-field post-editor__subject">
            <span>Subject</span>
            <input name="subject" maxlength="${escapeHtml(data.post_subject_max_length)}" required value="${escapeHtml(post.subject || "")}">
          </label>
          ${
            !showType
              ? ""
              : `
                <fieldset class="post-editor__type-options">
                  <legend>Type</legend>
                  ${Object.entries(postTypeLabels)
                    .map(
                      ([value, label]) => `
                        <label class="post-editor__type-choice ${postTypeClasses[value]}">
                          <input type="radio" name="post_type" value="${value}"${Number(post.post_type ?? 0) === Number(value) ? " checked" : ""}>
                          ${postTypeIcons[value]}
                          <span>${escapeHtml(label)}</span>
                        </label>
                      `,
                    )
                    .join("")}
                </fieldset>
              `
          }
          <label class="post-editor__encrypted">
            <input type="checkbox" name="encrypted"${Number(post.state ?? 0) === 1 ? " checked" : ""}>
            ${attachmentIcons.encrypted}
            <span>Encrypted</span>
          </label>
          <label class="login-field">
            <span>Content</span>
            <textarea name="content" rows="16">${escapeHtml(post.content || "")}</textarea>
          </label>
          ${
            isUpdate
              ? post.image_url
                ? `
                  <fieldset class="post-editor__resources post-editor__uploads">
                    <legend>Image</legend>
                    <p class="post-editor__attached">${attachmentIcons.image}<span>The attached image is retained and cannot be updated.</span></p>
                  </fieldset>
                `
                : ""
              : `
                <fieldset class="post-editor__resources post-editor__uploads">
                  <legend>Image</legend>
                  <label class="login-field post-editor__image">
                    <span>Image attachment</span>
                    <input type="file" name="image" accept="image/jpeg,image/png,image/gif">
                  </label>
                  <p class="post-editor__upload-note">Supported formats: JPG, PNG, GIF. Maximum file size: ${escapeHtml(formatFileSizeLimit(data.image_upload_max_bytes))}. Images larger than 500 KB are compressed below 500 KB for storage.</p>
                </fieldset>
              `
          }
          <p class="login-form__error" data-post-editor-error hidden></p>
          <div class="post-editor__commands">
            <button class="login-submit" type="submit">${isReply ? "Publish reply" : isCreate ? "Publish post" : "Save changes"}</button>
            <a class="password-change__cancel" href="${isCreate ? `/board/${encodeURIComponent(data.board.id)}` : isReply ? `/post/${encodeURIComponent(parent.id)}` : `/post/${encodeURIComponent(post.id)}`}">Cancel</a>
          </div>
        </form>
      </section>
    `;
  }

  bindPostEditor(data) {
    const form = this.querySelector("[data-post-editor-form]");
    const error = form.querySelector("[data-post-editor-error]");
    const showType = this.postEditorShowsType(data);
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      const fields = new FormData(form);
      const image = fields.get("image");
      const content = String(fields.get("content") || "");
      const button = form.querySelector("button[type='submit']");
      error.hidden = true;
      button.disabled = true;
      try {
        if (new TextEncoder().encode(content).length > Number(data.post_content_max_bytes)) {
          throw new Error("Post content exceeds the configured size limit.");
        }
        const saved = await submitPostSave({
          board_id: data.mode === "create" ? Number(data.board.id) : null,
          post_id: data.mode === "update" ? Number(data.post.id) : null,
          parent_id: data.mode === "reply" ? Number(data.parent.id) : null,
          subject: String(fields.get("subject") || ""),
          content,
          post_type: showType ? Number(fields.get("post_type") || 0) : null,
          state: fields.get("encrypted") ? 1 : 0,
        });
        if (image instanceof File && image.size > 0) {
          await submitPostImageUpload(saved.post_id, image);
        }
        window.location.assign(`/post/${encodeURIComponent(saved.post_id)}`);
      } catch (requestError) {
        error.textContent = requestError.message || "Unable to save post or upload image.";
        error.hidden = false;
        button.disabled = false;
      }
    });
  }

  postEditorShowsType(data) {
    return data.mode !== "reply" && (data.mode !== "update" || Number(data.post?.level || 0) === 0);
  }

  renderPostListPage(data) {
    const treePosts = [...data.posts].sort(
      (left, right) => Number(left.order_num || 0) - Number(right.order_num || 0),
    );
    return `
      ${this.renderPostController(data, data.selected_post_id, true)}
      <section class="post-list section--wide" aria-label="Posts in this tree">
        ${data.posts
          .map((post, index) =>
            this.renderPostDetail(post, {
              current: Number(post.id) === Number(data.selected_post_id),
              headingTag: index === 0 ? "h1" : "h2",
              linkTitle: true,
            }),
          )
          .join("")}
      </section>
      ${this.renderPostTree({ posts: treePosts }, data.selected_post_id)}
    `;
  }

  renderPostController(data, postId, listView = false) {
    return `
      <nav class="post-controller section section--wide" aria-label="Post controls">
        <a class="post-controller__board" href="/board/${encodeURIComponent(data.board.id)}">
          ${boardListIcon}
          <span>${escapeHtml(data.board.name)}</span>
        </a>
        ${
          listView
            ? ""
            : `
              <div class="post-controller__actions">
                <a class="tool-button" href="/post_list/${encodeURIComponent(postId)}" title="List view" aria-label="List view">
                  ${postActionIcons.list}
                </a>
                <a class="tool-button" href="/post_print/${encodeURIComponent(postId)}" target="_blank" rel="noopener" title="Print version" aria-label="Print version">
                  ${postActionIcons.print}
                </a>
                ${
                  data.can_update
                    ? `
                      <a class="tool-button" href="/post_upd?post_id=${encodeURIComponent(postId)}" title="Update post" aria-label="Update post">
                        ${postActionIcons.edit}
                      </a>
                    `
                    : ""
                }
                ${
                  data.can_delete
                    ? `
                      <button class="tool-button tool-button--danger" type="button" title="Delete post" aria-label="Delete post" data-post-delete-toggle>
                        ${postActionIcons.delete}
                      </button>
                    `
                    : ""
                }
                ${
                  data.can_reply
                    ? `
                      <a class="tool-button" href="/post_upd?reply_to=${encodeURIComponent(postId)}" title="Reply" aria-label="Reply">
                        ${postActionIcons.reply}
                      </a>
                    `
                    : `
                      <span class="post-controller__hint">${data.reply_open ? "Login to reply" : "Replies closed"}</span>
                    `
                }
              </div>
            `
        }
      </nav>
      ${
        !listView && data.can_delete
          ? `
            <section class="statistics-confirmation section section--wide" data-post-delete-confirmation hidden>
              <h2>Delete post</h2>
              <p>This post will become invisible to readers. Its stored record is retained.</p>
              <p class="login-form__error" data-post-delete-error hidden></p>
              <div class="post-editor__commands">
                <button class="login-submit" type="button" data-post-delete-confirm>Delete</button>
                <button class="password-change__cancel" type="button" data-post-delete-cancel>Cancel</button>
              </div>
            </section>
          `
          : ""
      }
    `;
  }

  bindPostActions(data) {
    const toggle = this.querySelector("[data-post-delete-toggle]");
    const confirmation = this.querySelector("[data-post-delete-confirmation]");
    if (!toggle || !confirmation) {
      return;
    }

    const confirm = confirmation.querySelector("[data-post-delete-confirm]");
    const cancel = confirmation.querySelector("[data-post-delete-cancel]");
    const error = confirmation.querySelector("[data-post-delete-error]");
    toggle.addEventListener("click", () => {
      confirmation.hidden = false;
      error.hidden = true;
      confirm.focus();
    });
    cancel.addEventListener("click", () => {
      confirmation.hidden = true;
      error.hidden = true;
      toggle.focus();
    });
    confirm.addEventListener("click", async () => {
      confirm.disabled = true;
      error.hidden = true;
      try {
        const result = await submitPostDeletion(data.post.id);
        window.location.assign(`/board/${encodeURIComponent(result.board_id)}`);
      } catch (requestError) {
        error.textContent = requestError.message || "Unable to delete post.";
        error.hidden = false;
        confirm.disabled = false;
      }
    });
  }

  revealPostListSelection(post) {
    if (!post || Number(post.level) === 0) {
      return;
    }

    const selected = this.querySelector("[data-selected-post]");
    if (!selected) {
      return;
    }

    selected.scrollIntoView({
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
      block: "center",
    });
    selected.classList.add("is-attention");
    window.setTimeout(() => selected.classList.remove("is-attention"), 1800);
  }

  renderPostDetail(post, options = {}) {
    const headingTag = options.headingTag === "h2" ? "h2" : "h1";
    const currentClass = options.current ? " is-selected" : "";
    const currentAttribute = options.current ? " data-selected-post" : "";
    const subject = options.linkTitle
      ? `<a href="/post/${encodeURIComponent(post.id)}">${postTitle(post)}</a>`
      : postTitle(post);
    const type = postTypeLabels[post.post_type] || "Post";
    const typeIcon = postTypeIcons[post.post_type] || postTypeIcons[0];
    const typeClass = postTypeClasses[post.post_type] || postTypeClasses[0];
    const author = post.user_name || (post.user_id ? `user ${post.user_id}` : null);
    const metadata = [
      author
        ? this.renderPostMetaItem(
            postMetaIcons.author,
            author,
            "Author",
            post.user_id ? `/user/${encodeURIComponent(post.user_id)}` : null,
            false,
            true,
          )
        : "",
      post.post_time ? this.renderPostMetaItem(postMetaIcons.time, post.post_time, "Posted") : "",
      post.size == null
        ? ""
        : this.renderPostMetaItem(postMetaIcons.size, `${post.size} bytes`, "Size"),
      this.renderPostMetaItem(postMetaIcons.views, post.access_count ?? 0, "Views"),
      this.renderPostMetaItem(postMetaIcons.replies, post.reply_count ?? 0, "Replies"),
    ];

    if (post.point) {
      metadata.push(this.renderPostMetaItem(postMetaIcons.points, post.point, "Points"));
    }

    return `
      <article class="post-detail section section--wide${currentClass}"${currentAttribute}>
        <header class="post-detail__header">
          <span class="item-card__icon item-card__icon--post ${typeClass}" title="${escapeHtml(type)}">${typeIcon}</span>
          <div class="post-detail__heading">
            <div class="item-card__title-row">
              <${headingTag}>${subject}</${headingTag}>
              ${renderPostStatusBar(post)}
            </div>
            <p class="item-card__meta post-meta">${metadata.join("")}</p>
          </div>
        </header>
        ${this.renderPostContent(post)}
      </article>
    `;
  }

  renderPrintPost(data) {
    const post = data.post;
    const siteName = siteNameFrom(data);
    const author = post.user_name || (post.user_id ? `user ${post.user_id}` : null);
    const details = [
      data.board?.name ? `Board: ${data.board.name}` : "",
      author ? `Author: ${author}` : "",
      post.post_time ? `Posted: ${post.post_time}` : "",
      post.size == null ? "" : `Size: ${post.size} bytes`,
      `Views: ${post.access_count ?? 0}`,
      `Replies: ${post.reply_count ?? 0}`,
      post.point ? `Points: ${post.point}` : "",
      Number(post.state) === 1 ? "Encrypted" : "",
    ];

    return `
      <article class="print-post">
        <header class="print-post__header">
          <h1>${postTitle(post)}</h1>
          <p class="print-post__meta">
            <span class="print-post__site">${brandIcon}<span>${escapeHtml(siteName)}</span></span>
            <span aria-hidden="true"> · </span>${meta(details)}
          </p>
        </header>
        ${this.renderPrintContent(post)}
      </article>
    `;
  }

  renderPostContent(post) {
    if (post.content_visible === false) {
      return `<div class="post-detail__body"><span class="encrypted-pill">${attachmentIcons.encrypted}<span>Encrypted</span></span></div>`;
    }

    const body = post.has_content
      ? `<div class="post-detail__body">${escapeHtml(post.content || "")}</div>`
      : `<div class="post-detail__body post-detail__body--empty"><span class="empty-content-pill">No content</span></div>`;

    return `
      ${body}
      ${this.renderPostResources(post)}
      ${this.renderSignature(post.signature)}
      ${this.renderPointAwards(post)}
    `;
  }

  renderPrintContent(post) {
    if (post.content_visible === false) {
      return `<div class="print-post__body"><span class="encrypted-pill">${attachmentIcons.encrypted}<span>Encrypted</span></span></div>`;
    }

    return `
      <div class="print-post__body">${escapeHtml(post.content || "")}</div>
      ${this.renderPrintResources(post)}
      ${this.renderPrintSignature(post.signature)}
      ${this.renderPrintPointAwards(post)}
    `;
  }

  renderPrintResources(post) {
    const linkUrl = safeResourceUrl(post.link_url);
    const imageResource = postImageResource(post.image_url);
    const link = linkUrl
      ? `<p>Link: <a href="${escapeHtml(linkUrl)}">${escapeHtml(post.link_name || linkUrl)}</a></p>`
      : "";
    let image = "";

    if (imageResource?.local) {
      image = `<img class="print-post__image" src="${escapeHtml(imageResource.url)}" alt="${escapeHtml(post.subject || "Post image")}">`;
    } else if (imageResource) {
      image = `<p>Image: <a href="${escapeHtml(imageResource.url)}">${escapeHtml(imageResource.url)}</a></p>`;
    }

    return link || image ? `<section class="print-post__resources">${link}${image}</section>` : "";
  }

  renderPrintSignature(signature) {
    return signature?.content
      ? `<aside class="print-post__signature">${escapeHtml(signature.content)}</aside>`
      : "";
  }

  renderPrintPointAwards(post) {
    if (!post.point || !post.point_awards?.length) {
      return "";
    }

    const awards = post.point_awards
      .map((award) => `${award.user_name || `user ${award.user_id}`} - ${award.point}`)
      .join("; ");
    return `<section class="print-post__points">Points: ${escapeHtml(awards)}</section>`;
  }

  renderPostResources(post) {
    const linkUrl = safeResourceUrl(post.link_url);
    const imageResource = postImageResource(post.image_url);
    const link = linkUrl
      ? `
        <a class="post-resource__link" href="${escapeHtml(linkUrl)}" target="_blank" rel="noopener noreferrer">
          ${postActionIcons.link}
          <span>${escapeHtml(post.link_name || linkUrl)}</span>
        </a>
      `
      : "";
    let image = "";

    if (imageResource?.local) {
      image = `<img class="post-resource__image" src="${escapeHtml(imageResource.url)}" alt="${escapeHtml(post.subject || "Post image")}" loading="lazy">`;
    } else if (imageResource) {
      image = `
        <a class="post-resource__external-image" href="${escapeHtml(imageResource.url)}" target="_blank" rel="noopener noreferrer">
          ${postActionIcons.externalImage}
          <span>Open attached image</span>
        </a>
      `;
    }

    if (!link && !image) {
      return "";
    }

    return `<div class="post-resources">${link}${image}</div>`;
  }

  renderSignature(signature) {
    if (!signature?.content) {
      return "";
    }

    return `<aside class="post-signature" aria-label="Signature">${escapeHtml(signature.content)}</aside>`;
  }

  renderPointAwards(post) {
    if (!post.point) {
      return "";
    }

    return `
      <section class="point-awards" aria-label="Point awards">
        <h2>${postMetaIcons.points}<span>Points</span></h2>
        ${
          post.point_awards?.length
            ? `
              <ul>
                ${post.point_awards
                  .map((award) => {
                    const user = award.user_name || `user ${award.user_id}`;
                    return `
                      <li>
                        <a class="point-awards__user" href="/user/${encodeURIComponent(award.user_id)}" target="_blank" rel="noopener">${escapeHtml(user)}</a>
                        <span class="point-pill">${escapeHtml(award.point)}</span>
                      </li>
                    `;
                  })
                  .join("")}
              </ul>
            `
            : `<p class="section__state">No point records.</p>`
        }
      </section>
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

  renderPostTrees(trees, openPostInNewWindow = false) {
    return trees.length
      ? trees.map((tree) => this.renderPostTree(tree, null, openPostInNewWindow)).join("")
      : `<section class="section section--wide"><p class="section__state">No posts.</p></section>`;
  }

  renderPostTree(tree, currentPostId = null, openPostInNewWindow = false) {
    return `
      <article class="post-tree-card section section--wide">
        ${tree.posts.map((post) => this.renderBoardPost(post, currentPostId, openPostInNewWindow)).join("")}
      </article>
    `;
  }

  renderBoardPost(post, currentPostId = null, openPostInNewWindow = false) {
    const author = post.user_name || (post.user_id ? `user ${post.user_id}` : null);
    const type = postTypeLabels[post.post_type] || "Post";
    const typeIcon = postTypeIcons[post.post_type] || postTypeIcons[0];
    const typeClass = postTypeClasses[post.post_type] || postTypeClasses[0];
    const statusBar = renderPostStatusBar(post);
    const level = Math.max(0, Number(post.level) || 0);
    const isRoot = level === 0;
    const iconClass = isRoot
      ? `item-card__icon item-card__icon--post ${typeClass}`
      : "item-card__icon item-card__icon--reply";
    const icon = isRoot ? typeIcon : replyIcon;
    const currentClass = Number(post.id) === Number(currentPostId) ? " is-current" : "";
    const linkTarget = openPostInNewWindow ? ' target="_blank" rel="noopener"' : "";
    const metadata = [
      author
        ? this.renderPostMetaItem(
            postMetaIcons.author,
            author,
            "Author",
            post.user_id ? `/user/${encodeURIComponent(post.user_id)}` : null,
            false,
            true,
          )
        : "",
      post.post_time ? this.renderPostMetaItem(postMetaIcons.time, post.post_time, "Posted") : "",
      post.size == null
        ? ""
        : this.renderPostMetaItem(postMetaIcons.size, `${post.size} bytes`, "Size"),
      this.renderPostMetaItem(postMetaIcons.views, post.access_count ?? 0, "Views"),
    ];

    if (isRoot) {
      metadata.push(this.renderPostMetaItem(postMetaIcons.replies, post.reply_count ?? 0, "Replies"));
      metadata.push(this.renderPostMetaItem(postMetaIcons.points, post.point ?? 0, "Points"));
    }

    if (isRoot && post.reply_time) {
      metadata.push(
        this.renderPostMetaItem(postMetaIcons.replyTime, post.reply_time, "Latest reply"),
      );
    }

    return `
      <div class="item-card board-post${currentClass}" style="--post-indent: ${Math.min(level, 8)}">
        <span class="${iconClass}" title="${escapeHtml(type)}">${icon}</span>
        <div class="item-card__content">
          <div class="item-card__title-row">
            <a class="item-card__title" href="/post/${post.id}"${linkTarget}>${postTitle(post)}</a>
            ${statusBar}
          </div>
          <p class="item-card__meta post-meta">${metadata.join("")}</p>
        </div>
      </div>
    `;
  }

  renderPostMetaItem(icon, value, label, href = null, showLabel = false, openNewWindow = false) {
    const accessibleText = `${label}: ${value}`;
    const target = openNewWindow ? ' target="_blank" rel="noopener"' : "";
    const valueContent = href
      ? `<a class="post-meta__link" href="${escapeHtml(href)}"${target}>${escapeHtml(value)}</a>`
      : `<span>${escapeHtml(value)}</span>`;
    const content = showLabel
      ? `<span class="post-meta__label">${escapeHtml(label)}:</span>${valueContent}`
      : valueContent;
    return `
      <span class="post-meta__item" title="${escapeHtml(accessibleText)}" aria-label="${escapeHtml(accessibleText)}">
        ${icon}
        ${content}
      </span>
    `;
  }
}

customElements.define("dogn-app-shell", DognAppShell);
