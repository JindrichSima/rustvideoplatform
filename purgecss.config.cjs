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
      // data-bs-theme attribute selectors (theme switching)
      /^\[data-bs-theme/,
      // Theme system
      "active",
      "dragover",
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
      // Theme cards
      /^theme-/,
      // Filter chips
      /^filter-/,
    ],
    greedy: [],
    // Vidstack player CSS variables (player loaded from CDN, so vars aren't
    // seen in scanned content but are consumed by vidstack's own stylesheets)
    variables: [/^--video-/, /^--media-/, /^--glass-/, /^--surface-/, /^--text-/, /^--accent-/, /^--color-/, /^--radius-/, /^--transition-/, /^--font-/, /^--scrollbar-/],
  },
  // Preserve CSS variables and keyframes
  variables: true,
  keyframes: true,
  fontFace: true,
};
