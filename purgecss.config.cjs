module.exports = {
  content: [
    "templates/**/*.html",
    "assets/static/**/*.js",
  ],
  css: ["assets/static/style.css"],
  // Keep selectors that are dynamically added or used by third-party libraries
  safelist: {
    standard: [
      // Dynamically toggled via JS
      "sidebar-open",
      "sidebar-collapsed",
      "sidebar-animating",
      "sidebar-will-collapse",
      "fa-expand",
      "fa-compress",
      "fa-sun",
      "fa-moon",
      // Theme toggle icon classes
      "theme-icon-light",
      "theme-icon-dark",
      "theme-toggle-btn",
      // data-bs-theme attribute selectors (theme switching)
      /^\[data-bs-theme/,
    ],
    deep: [
      // Quill editor classes (dynamically generated)
      /^ql-/,
      // Highlight.js (dynamically generated)
      /^hljs/,
      // Tom Select (dynamically generated)
      /^ts-/,
      // HTMX (dynamically added)
      /^htmx-/,
      // Theme-scoped selectors
      /data-bs-theme/,
    ],
    greedy: [],
    // Vidstack player CSS variables (player loaded from CDN, so vars aren't
    // seen in scanned content but are consumed by vidstack's own stylesheets)
    variables: [/^--video-/, /^--media-/, /^--accent-/, /^--glass-/],
  },
  // Preserve CSS variables and keyframes
  variables: true,
  keyframes: true,
  fontFace: true,
};
