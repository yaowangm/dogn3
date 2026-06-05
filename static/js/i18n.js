(() => {
  const dictionaries = {
    en: {},
    zh: {
      "(untitled)": "(无标题)",
      "0 only": "仅 0",
      "Active": "活跃",
      "Activities": "活动",
      "Activity pagination": "活动分页",
      "Add board master": "添加版主",
      "Add category": "添加分类",
      "Add post": "发帖",
      "Add user": "添加用户",
      "Add board": "添加版面",
      "Added to favorites.": "已加入收藏。",
      "Administrator": "管理员",
      "Administrator access required": "需要管理员权限",
      "Administrator reset: the current password is not required.": "管理员重置：不需要当前密码。",
      "Advanced": "高级用户",
      "Advanced is assigned automatically while a member is a board master. Administrator and frozen roles are not changed by board-master assignment.": "用户担任版主时会自动成为高级用户。管理员和冻结角色不会因版主任命而改变。",
      "All roles": "全部角色",
      "Announce": "公告",
      "Any type": "全部类型",
      "Author": "作者",
      "Back to login": "返回登录",
      "Board": "版面",
      "Board name": "版面名称",
      "Board operations": "版面操作",
      "Board pagination": "版面分页",
      "Board statistics": "版面统计",
      "Boards": "版面",
      "Boards and categories": "版面和分类",
      "Boards loading...": "正在加载版面...",
      "Cancel": "取消",
      "Category": "分类",
      "Category name": "分类名称",
      "Change password": "修改密码",
      "Clear": "清除",
      "Confirm new password": "确认新密码",
      "Confirm password": "确认密码",
      "Content": "内容",
      "Content format": "内容格式",
      "Content keyword": "内容关键词",
      "Copied.": "已复制。",
      "Copy password": "复制密码",
      "Created from": "创建时间起",
      "Created to": "创建时间止",
      "Current password": "当前密码",
      "Current signature": "当前签名",
      "Delete": "删除",
      "Delete board": "删除版面",
      "Delete category": "删除分类",
      "Delete entire post tree": "删除整棵帖子树",
      "Delete post": "删除帖子",
      "Delete tree": "删除整棵树",
      "Description": "描述",
      "Email": "邮箱",
      "Email address": "邮箱地址",
      "Encrypted": "加密",
      "Encrypted post": "加密帖子",
      "Enter a new password for your account.": "请输入账号的新密码。",
      "Enter a user name or email.": "请输入用户名或邮箱。",
      "Enter a user name to search.": "请输入用户名搜索。",
      "Enter your forum user name and password.": "请输入论坛用户名和密码。",
      "Exit": "退出",
      "Favorite posts": "收藏帖子",
      "Favorites": "收藏",
      "First": "首页",
      "Forum": "论坛",
      "Forum overview": "论坛概览",
      "Forward": "转发",
      "Frozen": "冻结",
      "Generate new password": "生成新密码",
      "Has image attachment": "有图片附件",
      "Has related link": "有相关链接",
      "ID": "ID",
      "Image": "图片",
      "Image attachment": "图片附件",
      "Image not found": "图片未找到",
      "Introduction": "简介",
      "Introduced by": "介绍人",
      "Introducing user": "介绍人",
      "Invalid user name or password.": "用户名或密码无效。",
      "Last": "末页",
      "Last login": "最后登录",
      "Last login IP": "最后登录 IP",
      "Latest reply": "最后回复",
      "Link": "链接",
      "List view": "列表视图",
      "Loading...": "正在加载...",
      "Loading forum data...": "正在加载论坛数据...",
      "Loading post...": "正在加载帖子...",
      "Loading post editor...": "正在加载帖子编辑器...",
      "Loading post tree...": "正在加载帖子树...",
      "Loading search results...": "正在加载搜索结果...",
      "Loading site management data...": "正在加载站点管理数据...",
      "Loading user...": "正在加载用户...",
      "Loading users...": "正在加载用户...",
      "Login": "登录",
      "Login required": "需要登录",
      "Login is required to search posts.": "需要登录才能搜索帖子。",
      "Login is required to write a post.": "需要登录才能发帖。",
      "Login to reply": "登录后回复",
      "Login with your new password": "使用新密码登录",
      "Markdown": "Markdown",
      "Markdown supports headings, tables, math formulas, lists, quotes, code, emphasis, and safe links. Raw HTML is shown as text.": "Markdown 支持标题、表格、数学公式、列表、引用、代码、强调和安全链接。原始 HTML 会按文本显示。",
      "Markdown preview": "Markdown 预览",
      "Member": "普通用户",
      "New password": "新密码",
      "New users": "新用户",
      "Newest ID first": "ID 从新到旧",
      "Next": "下一页",
      "No board masters assigned.": "未设置版主。",
      "No boards.": "没有版面。",
      "No boards in this category.": "此分类下没有版面。",
      "No content": "无内容",
      "No introducing user selected.": "未选择介绍人。",
      "No matching users.": "没有匹配用户。",
      "No point records.": "没有积分记录。",
      "No posts.": "没有帖子。",
      "No posts found.": "没有找到帖子。",
      "No unassigned matching users.": "没有未分配的匹配用户。",
      "No users.": "没有用户。",
      "Normal": "普通",
      "Oldest ID first": "ID 从旧到新",
      "Open attached image": "打开附件图片",
      "Operations": "操作",
      "Order": "排序",
      "Original": "原创",
      "Original posts": "原创帖子",
      "Originals": "原创",
      "Page shell loaded, but the JSON API did not respond successfully.": "页面外壳已加载，但 JSON API 未成功响应。",
      "The page shell loaded, but the JSON API did not respond successfully.": "页面外壳已加载，但 JSON API 未成功响应。",
      "Password": "密码",
      "Password changed.": "密码已修改。",
      "Plain text": "纯文本",
      "Point awards": "积分奖励",
      "Points": "积分",
      "Points to author": "转给作者的积分",
      "Portal": "门户",
      "Post": "帖子",
      "Post controls": "帖子控制",
      "Post editor navigation": "帖子编辑导航",
      "Post content format cannot be changed after publication.": "帖子发布后不能修改内容格式。",
      "Post search": "帖子搜索",
      "Post search controls": "帖子搜索控制",
      "Post status": "帖子状态",
      "PGroonga Chinese/multilingual full-text search": "PGroonga 中文/多语言全文搜索",
      "Post unavailable": "帖子不可用",
      "PostgreSQL-backed Rust web application": "基于 PostgreSQL 的 Rust Web 应用",
      "Posted": "发布时间",
      "Posts": "帖子",
      "Posts in this tree": "此树中的帖子",
      "Previous": "上一页",
      "Preview": "预览",
      "Print": "打印",
      "Print version": "打印版",
      "Profile": "资料",
      "Profile operations": "资料操作",
      "Publish post": "发布帖子",
      "Publish reply": "发布回复",
      "Recalculate": "重新计算",
      "Recalculate board statistics": "重新计算版面统计",
      "Recalculate statistics": "重新计算统计",
      "Recent announcement": "最新公告",
      "Recent announcement posts": "最新公告帖",
      "Recent discussions, original posts, forwards, users, and boards.": "最新讨论、原创、转发、用户和版面。",
      "Recent forward posts": "最新转发帖",
      "Recent original posts": "最新原创帖",
      "Recent root posts": "最新主题帖",
      "Registered": "注册时间",
      "Removed from favorites.": "已取消收藏。",
      "Remove": "移除",
      "Replies": "回复",
      "Replies closed": "回复已关闭",
      "Reply": "回复",
      "Replied from": "回复时间起",
      "Replied to": "回复时间止",
      "Reset password": "重置密码",
      "Role": "角色",
      "Save board": "保存版面",
      "Save category": "保存分类",
      "Save changes": "保存修改",
      "Search": "搜索",
      "Search for a user to assign as board master.": "搜索用户并设置为版主。",
      "Search pagination": "搜索分页",
      "Search posts": "搜索帖子",
      "Search results": "搜索结果",
      "Search time": "搜索耗时",
      "SQL search method": "SQL 搜索方法",
      "Search user name or email": "搜索用户名或邮箱",
      "Searching users...": "正在搜索用户...",
      "Send reset email": "发送重置邮件",
      "Set favorite": "收藏",
      "Set role": "设置角色",
      "Set signature": "设为签名",
      "Signature": "签名",
      "Signature posts": "签名帖子",
      "Signature updated.": "签名已更新。",
      "Site manager": "站点管理",
      "Site manager operations": "站点管理操作",
      "Size": "大小",
      "Subject": "标题",
      "Subject keyword": "标题关键词",
      "Suggested password": "建议密码",
      "The editor target is unavailable.": "编辑目标不可用。",
      "The page shell loaded, but the board JSON API did not respond successfully.": "页面外壳已加载，但版面 JSON API 未成功响应。",
      "The page shell loaded, but the post JSON API did not respond successfully.": "页面外壳已加载，但帖子 JSON API 未成功响应。",
      "The page shell loaded, but the post list JSON API did not respond successfully.": "页面外壳已加载，但帖子列表 JSON API 未成功响应。",
      "The page shell loaded, but the search JSON API did not respond successfully.": "页面外壳已加载，但搜索 JSON API 未成功响应。",
      "The page shell loaded, but the site-manager JSON API did not respond successfully.": "页面外壳已加载，但站点管理 JSON API 未成功响应。",
      "The page shell loaded, but the user JSON API did not respond successfully.": "页面外壳已加载，但用户 JSON API 未成功响应。",
      "The page shell loaded, but the user-list JSON API did not respond successfully.": "页面外壳已加载，但用户列表 JSON API 未成功响应。",
      "The password reset link is missing its token.": "密码重置链接缺少令牌。",
      "The print version could not be loaded.": "无法加载打印版。",
      "This page is available only to administrators.": "此页面仅管理员可访问。",
      "This post does not exist or has been deleted.": "此帖子不存在或已被删除。",
      "This post is no longer open for replies.": "此帖子已关闭回复。",
      "This post will become invisible to readers. Its stored record is retained.": "此帖子将对读者不可见，但数据库记录会保留。",
      "This succeeds only if the board contains no posts.": "仅当版面没有帖子时才能成功。",
      "This succeeds only if the category contains no boards.": "仅当分类没有版面时才能成功。",
      "This user does not exist.": "此用户不存在。",
      "Top point users": "积分最高用户",
      "Type": "类型",
      "Unable to add user.": "无法添加用户。",
      "Unable to copy automatically. Select the displayed password to copy it.": "无法自动复制。请选择显示的密码进行复制。",
      "Unable to delete post.": "无法删除帖子。",
      "Unable to load board data": "无法加载版面数据",
      "Unable to load forum data": "无法加载论坛数据",
      "Unable to load post": "无法加载帖子",
      "Unable to load post data": "无法加载帖子数据",
      "Unable to load post editor": "无法加载帖子编辑器",
      "Unable to load post tree": "无法加载帖子树",
      "Unable to load search results": "无法加载搜索结果",
      "Unable to load site manager": "无法加载站点管理",
      "Unable to load user data": "无法加载用户数据",
      "Unable to load user list": "无法加载用户列表",
      "Unable to request a password reset.": "无法请求密码重置。",
      "Unable to reset password.": "无法重置密码。",
      "Unable to save post or upload image.": "无法保存帖子或上传图片。",
      "Unable to search users.": "无法搜索用户。",
      "Unable to update favorites.": "无法更新收藏。",
      "Unable to update signature.": "无法更新签名。",
      "Unset favorite": "取消收藏",
      "Updated": "更新时间",
      "Update": "更新",
      "Update not permitted": "不允许更新",
      "Update post": "更新帖子",
      "Update profile": "更新资料",
      "User activities": "用户活动",
      "User unavailable": "用户不可用",
      "User list": "用户列表",
      "User list controls": "用户列表控制",
      "User name": "用户名",
      "User name keyword": "用户名关键词",
      "User name or email": "用户名或邮箱",
      "User statistics": "用户统计",
      "User status": "用户状态",
      "Users": "用户",
      "Views": "浏览",
      "Warning": "警告",
      "You do not have permission to update this post.": "你没有权限更新此帖子。",
      "bytes": "字节",
      "docs": "原创",
      "favorites": "收藏",
      "login": "登录",
      "points": "积分",
      "posts": "帖子",
      "roots": "主题",
      "users": "用户",
      "views": "浏览",
    },
  };

  const keyed = {
    en: {
      "common.loading": "Loading...",
      "post_editor.root_post_award_hint": "Daily root-post award: regular/announcement +{regular}, forward +{forward}, original +{original}. Each type awards once per database day.",
    },
    zh: {
      "common.loading": "正在加载...",
      "post_editor.root_post_award_hint": "每日主题帖奖励：普通/公告 +{regular}，转发 +{forward}，原创 +{original}。每类按数据库日期每天只奖励一次。",
    },
  };

  const attributeNames = ["aria-label", "title", "placeholder"];
  const skipSelector = [
    "[data-no-i18n]",
    ".item-card__title",
    ".post-detail__body:not(.post-detail__body--empty)",
    ".print-post__body",
    ".post-signature",
    ".print-post__signature",
    ".user-profile__signature",
    ".user-profile__text",
    "textarea",
    "input",
    "code",
    "pre",
    "script",
    "style",
  ].join(",");

  function detectLanguage() {
    const candidates = Array.isArray(navigator.languages) && navigator.languages.length
      ? navigator.languages
      : [navigator.language || "en"];

    for (const candidate of candidates) {
      const language = String(candidate || "").toLowerCase();
      if (language === "zh" || language.startsWith("zh-")) {
        return "zh";
      }
      if (language === "en" || language.startsWith("en-")) {
        return "en";
      }
    }

    return "en";
  }

  const language = detectLanguage();

  function translateExact(value) {
    const text = String(value ?? "");
    if (language === "en" || !text.trim()) {
      return text;
    }

    const leading = text.match(/^\s*/)?.[0] || "";
    const trailing = text.match(/\s*$/)?.[0] || "";
    const core = text.trim();
    const translated = dictionaries[language]?.[core] ?? dictionaries.en?.[core];
    return translated ? `${leading}${translated}${trailing}` : text;
  }

  function translatePattern(value) {
    let text = translateExact(value);
    if (language !== "zh") {
      return text;
    }

    text = text
      .replace(/^Open (.+) menu$/, "打开 $1 菜单")
      .replace(/^Author: (.+)$/, "作者：$1")
      .replace(/^Posted: (.+)$/, "发布时间：$1")
      .replace(/^Updated: (.+)$/, "更新时间：$1")
      .replace(/^Size: (.+)$/, "大小：$1")
      .replace(/^Views: (.+)$/, "浏览：$1")
      .replace(/^Replies: (.+)$/, "回复：$1")
      .replace(/^Points: (.+)$/, "积分：$1")
      .replace(/^Latest reply: (.+)$/, "最后回复：$1")
      .replace(/^Last login IP: (.+)$/, "最后登录 IP：$1")
      .replace(/^Email: (.+)$/, "邮箱：$1")
      .replace(/^Logins: (.+)$/, "登录次数：$1")
      .replace(/^Joined: (.+)$/, "注册时间：$1")
      .replace(/^ID (.+)$/, "ID $1")
      .replace(/^Page (.+) \/ (.+)$/, "第 $1 / $2 页")
      .replace(/^Page (.+) \/ (.+) \((.+) users\)$/, "第 $1 / $2 页（$3 个用户）")
      .replace(/^Page (.+) \/ (.+) \((.+) posts\)$/, "第 $1 / $2 页（$3 个帖子）")
      .replace(/^(.+) posts found\.$/, "找到 $1 个帖子。")
      .replace(/^Reply to: (.+)$/, "回复：$1")
      .replace(/^Delete category (.+)\?$/, "删除分类 $1？")
      .replace(/^Add board to (.+)$/, "添加版面到 $1")
      .replace(/^Delete board (.+)\?$/, "删除版面 $1？")
      .replace(/^Change password for (.+)$/, "修改 $1 的密码")
      .replace(/^Recalculate statistics for (.+)\?$/, "重新计算 $1 的统计？")
      .replace(/^Set role for (.+)$/, "设置 $1 的角色")
      .replace(/^Update profile for (.+)$/, "更新 $1 的资料")
      .replace(/^Points to author \((.+)\)$/, "转给作者的积分（$1）")
      .replace(/^(\d+) bytes$/, "$1 字节")
      .replace(/^Link:$/, "链接：")
      .replace(/^Image:$/, "图片：")
      .replace(/^Points:$/, "积分：");
    return text;
  }

  function interpolate(template, values) {
    return String(template).replace(/\{([a-zA-Z0-9_]+)\}/g, (match, key) =>
      Object.prototype.hasOwnProperty.call(values, key) ? String(values[key]) : match,
    );
  }

  function t(key, values = {}, fallback = "") {
    const template =
      keyed[language]?.[key] ??
      keyed.en?.[key] ??
      dictionaries[language]?.[key] ??
      dictionaries.en?.[key] ??
      fallback ??
      key;
    return interpolate(template, values);
  }

  function shouldSkipTextNode(node) {
    const parent = node.parentElement;
    if (!parent) {
      return true;
    }
    if (parent.closest(skipSelector)) {
      return true;
    }
    return !node.nodeValue.trim();
  }

  function translateTextNode(node) {
    if (shouldSkipTextNode(node)) {
      return;
    }
    const translated = translatePattern(node.nodeValue);
    if (translated !== node.nodeValue) {
      node.nodeValue = translated;
    }
  }

  function translateAttributes(element) {
    for (const attribute of attributeNames) {
      if (!element.hasAttribute(attribute)) {
        continue;
      }
      const current = element.getAttribute(attribute);
      const translated = translatePattern(current);
      if (translated !== current) {
        element.setAttribute(attribute, translated);
      }
    }
  }

  function translateElement(root) {
    if (language === "en" || !root) {
      return;
    }

    if (root.nodeType === Node.TEXT_NODE) {
      translateTextNode(root);
      return;
    }

    if (root.nodeType !== Node.ELEMENT_NODE && root.nodeType !== Node.DOCUMENT_NODE) {
      return;
    }

    const element = root.nodeType === Node.ELEMENT_NODE ? root : null;
    if (element) {
      if (element.matches(skipSelector)) {
        return;
      }
      translateAttributes(element);
    }

    for (const child of Array.from(root.childNodes || [])) {
      translateElement(child);
    }
  }

  function startObserver() {
    document.documentElement.lang = language === "zh" ? "zh-CN" : "en";

    if (language === "en") {
      return;
    }

    translateElement(document.body);

    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        if (mutation.type === "attributes") {
          translateAttributes(mutation.target);
          continue;
        }
        if (mutation.type === "characterData") {
          translateTextNode(mutation.target);
          continue;
        }
        for (const node of mutation.addedNodes) {
          translateElement(node);
        }
      }
    });

    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      characterData: true,
      attributeFilter: attributeNames,
    });
  }

  window.dognI18n = {
    language,
    t,
    translateElement,
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", startObserver, { once: true });
  } else {
    startObserver();
  }
})();
