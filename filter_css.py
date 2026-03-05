#!/usr/bin/env python3
"""Filter style.css to only include rules for classes/IDs actually used in templates."""

import re
import sys

# All CSS classes actually used in templates
USED_CLASSES = {
    'align-items-center', 'align-items-end', 'align-items-start',
    # 'active' excluded: too generic, causes false matches (e.g. .carousel-item.active)
    # Rules using .filter-chip.active are already caught via .filter-chip
    'bg-hover',
    'border',
    'btn', 'btn-danger', 'btn-lg', 'btn-outline-danger', 'btn-primary', 'btn-secondary', 'btn-sm',
    'card',
    'channel-picture-container',
    'col-12', 'col-4', 'col-6', 'col-8',
    'col-form-label',
    'col-lg-12', 'col-lg-3', 'col-lg-4', 'col-lg-5', 'col-lg-6', 'col-lg-9',
    'col-md-12', 'col-md-4', 'col-md-6', 'col-md-8',
    'col-sm', 'col-sm-12', 'col-sm-3', 'col-sm-4', 'col-sm-5',
    'col-xl-2', 'col-xl-3', 'col-xl-4',
    'comment-content', 'comment-item',
    'd-block', 'd-flex', 'd-inline-block', 'd-none',
    'description-collapsed', 'description-content', 'description-fade', 'description-wrapper',
    'dropdown', 'dropdown-content', 'dropdown-item',
    'filter-chip', 'filter-chips', 'filter-group', 'filter-label',
    'flex-column', 'flex-shrink-0',
    'font-weight-bold',
    'form-control', 'form-control-sm', 'form-group', 'form-label', 'form-select', 'form-select-sm',
    'fs-2', 'fs-3', 'fs-5',
    'fw-bold', 'fw-semibold',
    'g-2', 'g-3',
    'gap-2', 'gap-3',
    'h-100',
    'htmx-indicator',
    'htmx-request',
    'hx-placeholder',
    'inf-scroll-placeholder', 'inf-scroll-reveal',
    'input-group', 'input-group-append',
    'justify-content-between', 'justify-content-center',
    'list-group', 'list-group-horizontal',
    'list-modal-content', 'list-modal-overlay',
    'list-sidebar-active',
    'list-unstyled',
    'm-0', 'm-2', 'm-3',
    'maincontentrow',
    'mb-0', 'mb-2', 'mb-3', 'mb-auto',
    'me-1', 'me-2', 'me-3', 'me-auto',
    'medium-channel-col', 'medium-engagement-col', 'medium-info-grid', 'medium-name',
    'medium-pdf', 'medium-picture', 'medium-select-card', 'medium-title-col', 'medium-title-wrapper',
    'ms-1', 'ms-2', 'ms-3',
    'mt-0', 'mt-1', 'mt-2', 'mt-3', 'mt-4',
    'mx-1', 'mx-2', 'mx-3', 'mx-4',
    'my-1', 'my-2', 'my-3', 'my-4',
    'nav', 'nav-item', 'nav-link', 'nav-pills', 'nav-tabs',
    'navbar-container', 'navbar-instance-name', 'navbar-mobile-title', 'navbar-page-title',
    'navbar-search-form', 'navbar-top-row',
    'offset-4',
    'p-0', 'p-1', 'p-2', 'p-3',
    'pdf-fullscreen-btn', 'pdf-viewer-wrapper',
    'player-container',
    'ps-2', 'pt-3',
    'px-0', 'px-2', 'px-3',
    'py-2', 'py-3',
    # Quill editor (ql-editor is used; include all ql-* for the editor to work)
    'ql-editor', 'ql-container', 'ql-snow', 'ql-toolbar', 'ql-clipboard',
    'ql-blank', 'ql-active', 'ql-disabled', 'ql-image',
    'rounded',
    'row',
    'search-bar-wrapper', 'search-card-info', 'search-card-likes', 'search-card-meta',
    'search-card-owner', 'search-card-thumbnail', 'search-card-title', 'search-card-type-badge',
    'search-card-views',
    'search-empty',
    'search-filters', 'search-header', 'search-input-group', 'search-loading', 'search-loading-bar',
    'search-main-input', 'search-meta', 'search-meta-count', 'search-meta-time',
    'search-result-card', 'search-results-grid', 'search-submit-btn',
    'searchbtn', 'searchinput',
    'shadow',
    'sidebar', 'sidebar-animating', 'sidebar-collapsed', 'sidebar-hx-placeholder', 'sidebar-open',
    'sidebarbackground',
    'subscribe-placeholder',
    'suggestionitem',
    'table', 'table-bordered', 'table-hover', 'table-responsive',
    'text-center', 'text-danger', 'text-decoration-none', 'text-primary', 'text-secondary',
    'text-success', 'text-truncate', 'text-white',
    'thumbnail-animated', 'thumbnail-container', 'thumbnail-static',
    'vds-poster',
    'w-100',
    # video.js (used by video player)
    'video-js',
}

# All IDs actually used in templates
USED_IDS = {
    'add-chapter-btn', 'channel_name', 'chapters-editor', 'chapters-status', 'chapters-table', 'chapters-tbody',
    'codec-av1', 'codec-h265', 'codec-opus', 'codec-vp9',
    'comment_editor', 'comment_submit_btn', 'comment_text', 'comments-list',
    'confirm_password', 'current_password',
    'dateFilter', 'dateRangeInput',
    'description_fade', 'description_toggle', 'description_wrapper',
    'edit-form', 'form',
    'group-members-panel', 'groups-list',
    'listModalOverlay', 'listmodal-body', 'listmodal-group-select', 'listmodal-visibility',
    'login', 'logininfo',
    'mediaTypeInput', 'medium-title',
    'medium_description', 'medium_description_editor', 'medium_description_view',
    'medium_id', 'medium_id_display', 'medium_name', 'medium_restricted_group', 'medium_visibility',
    'navSearchForm', 'new_password', 'no-chapters-msg', 'no-groups-hint',
    'password',
    'pdfFullscreenIcon', 'pdfViewerWrapper',
    'progress', 'progressBar', 'progressBarContainer',
    'publish-form',
    'save-chapters-btn',
    'search-loading', 'searchForm', 'searchInput', 'searchMainInput', 'searchresults',
    'settings-result',
    'sidebar', 'sidebarbackground',
    'sortByInput', 'sortFilter',
    'submitbutton',
    'subtitle-file-input', 'subtitle-label-input', 'subtitles-editor', 'subtitles-status',
    'subtitles-table', 'subtitles-tbody',
    'suggestions',
    'tab-content',
    'typeFilter',
    'upload-container', 'upload-form', 'upload-progress', 'upload-result', 'upload-submitbutton',
}

# @keyframes names used in the custom CSS we're keeping
USED_KEYFRAMES = {'infScrollFadeIn', 'search-loading-slide'}

# Prefix patterns: if a selector class starts with these, include all (e.g. ql-*)
USED_PREFIXES = ('ql-',)


def selector_uses_identifier(selector: str) -> bool:
    """Return True if the selector references any used class or ID."""
    classes = set(re.findall(r'\.(-?[a-zA-Z][a-zA-Z0-9_-]*)', selector))
    ids = set(re.findall(r'#([a-zA-Z][a-zA-Z0-9_-]*)', selector))

    for cls in classes:
        if cls in USED_CLASSES:
            return True
        if any(cls.startswith(p) for p in USED_PREFIXES):
            return True
    for ident in ids:
        if ident in USED_IDS:
            return True
    return False


def is_global_selector(selector: str) -> bool:
    """Return True for universal/base selectors that should always be included."""
    stripped = selector.strip()
    global_patterns = [
        r'^\*$',
        r'^\*::?(?:before|after)$',
        r'^html$',
        r'^body$',
        r'^html\s*,\s*body$',
        r'^:root$',
        r'^\[data-bs-theme[^\]]*\]$',  # standalone [data-bs-theme="..."] only
        r'^::-webkit-scrollbar',
        r'^@charset',
    ]
    return any(re.match(p, stripped) for p in global_patterns)


def should_include_selector_list(selector_list: str) -> bool:
    """
    Handle comma-separated selectors.
    Include a rule if ANY part of the selector list matches.
    """
    parts = selector_list.split(',')
    for part in parts:
        part = part.strip()
        if is_global_selector(part):
            return True
        if selector_uses_identifier(part):
            return True
    return False


def find_block_end(text: str, start: int) -> int:
    """Find the index after the closing brace, handling nested braces."""
    depth = 0
    i = start
    while i < len(text):
        if text[i] == '{':
            depth += 1
        elif text[i] == '}':
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return len(text)


def process_at_media(at_header: str, inner_text: str) -> str | None:
    """Filter an @media (or @supports) block's inner rules.

    at_header already includes the opening '{'.
    """
    filtered = filter_rules(inner_text)
    if filtered.strip():
        return f"{at_header}\n{filtered}\n}}"
    return None


def filter_rules(css: str) -> str:
    """
    Walk through css text and return only the rules whose selectors
    reference used classes/IDs (or are global/base selectors).
    """
    output = []
    i = 0
    n = len(css)

    while i < n:
        # Skip whitespace
        while i < n and css[i] in ' \t\n\r':
            i += 1
        if i >= n:
            break

        # @-rule
        if css[i] == '@':
            # Read until '{' or ';'
            j = i
            while j < n and css[j] not in ('{', ';'):
                j += 1

            if j >= n:
                break

            if css[j] == ';':
                # Simple @-rule: @charset, @import, etc.
                rule_text = css[i:j + 1].strip()
                # Always keep @charset
                if rule_text.startswith('@charset'):
                    output.append(rule_text)
                i = j + 1
                continue

            # Block @-rule
            at_header = css[i:j + 1]  # includes '{'
            block_end = find_block_end(css, j)
            inner_text = css[j + 1:block_end - 1]  # content between outer braces

            at_name_match = re.match(r'@(-?[a-zA-Z-]+)', at_header)
            at_name = at_name_match.group(1).lower() if at_name_match else ''

            if at_name in ('keyframes', '-webkit-keyframes'):
                kf_name_match = re.search(r'@(?:-webkit-)?keyframes\s+([^\s{]+)', at_header)
                kf_name = kf_name_match.group(1) if kf_name_match else ''
                if kf_name in USED_KEYFRAMES:
                    output.append(css[i:block_end])

            elif at_name in ('media', 'supports', 'layer'):
                result = process_at_media(at_header, inner_text)
                if result:
                    output.append(result)

            else:
                # @font-face, @page, etc. — include as-is (they don't have class selectors)
                output.append(css[i:block_end])

            i = block_end
            continue

        # Regular rule: read selector up to '{'
        j = i
        while j < n and css[j] != '{':
            j += 1
        if j >= n:
            break

        selector = css[i:j].strip()
        block_end = find_block_end(css, j)
        block = css[j:block_end]  # '{...}'

        if should_include_selector_list(selector):
            output.append(f"{selector} {block}")

        i = block_end

    return '\n\n'.join(output)


def main():
    input_file = sys.argv[1]
    output_file = sys.argv[2]

    with open(input_file, 'r', encoding='utf-8') as f:
        css = f.read()

    filtered = filter_rules(css)

    with open(output_file, 'w', encoding='utf-8') as f:
        f.write(filtered)

    orig_lines = css.count('\n')
    new_lines = filtered.count('\n')
    orig_kb = len(css) / 1024
    new_kb = len(filtered) / 1024
    reduction_pct = (1 - new_kb / orig_kb) * 100

    print(f"Original : {orig_lines:,} lines  ({orig_kb:.1f} KB)")
    print(f"Filtered : {new_lines:,} lines  ({new_kb:.1f} KB)")
    print(f"Reduction: {reduction_pct:.1f}%")


if __name__ == '__main__':
    main()
