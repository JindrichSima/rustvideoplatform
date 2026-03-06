// Critical: run immediately to set theme and sidebar state before paint
(function(){var d=window.matchMedia&&window.matchMedia('(prefers-color-scheme: dark)').matches;document.documentElement.setAttribute('data-bs-theme',d?'dark':'light');})();
(function(){try{if(localStorage.getItem('sidebar-collapsed')==='1'){document.documentElement.classList.add('sidebar-will-collapse');}}catch(e){}})();

"use strict";

const MOBILE_QUERY = "(max-width: 1000px)";

function toggleSidebar() {
    if (window.matchMedia(MOBILE_QUERY).matches) {
        document.getElementById("sidebar").classList.toggle("sidebar-open");
        document.getElementById("sidebarbackground").classList.toggle("sidebar-open");
    } else {
        document.body.classList.add("sidebar-animating");
        document.body.classList.toggle("sidebar-collapsed");
        try {
            localStorage.setItem("sidebar-collapsed", document.body.classList.contains("sidebar-collapsed") ? "1" : "0");
        } catch (e) {}
        setTimeout(() => {
            document.body.classList.remove("sidebar-animating");
        }, 300);
    }
}

function navbarSearch(event) {
    event.preventDefault();
    const input = document.getElementById('searchInput');
    const query = input && input.value.trim();
    if (query) {
        window.location.href = '/search?q=' + encodeURIComponent(query);
    }
    return false;
}

document.addEventListener('DOMContentLoaded', () => {
    const docEl = document.documentElement;
    if (docEl.classList.contains('sidebar-will-collapse')) {
        document.body.classList.add('sidebar-collapsed');
        docEl.classList.remove('sidebar-will-collapse');
    }

    const sidebarBg = document.getElementById("sidebarbackground");
    if (sidebarBg) {
        sidebarBg.addEventListener("click", function () {
            if (window.matchMedia(MOBILE_QUERY).matches) {
                document.getElementById("sidebar").classList.remove("sidebar-open");
                this.classList.remove("sidebar-open");
            }
        });
    }

    const searchInput = document.getElementById('searchInput');
    const suggestionsList = document.getElementById('suggestions');

    if (searchInput && suggestionsList) {
        searchInput.addEventListener('focus', () => {
            if (suggestionsList.children.length > 0) {
                suggestionsList.style.display = '';
            }
        });

        searchInput.addEventListener('blur', () => {
            setTimeout(() => {
                suggestionsList.style.display = 'none';
            }, 250);
        });
    }

    document.body.addEventListener('htmx:afterSwap', (event) => {
        const target = event.detail.target;
        if (target.id === 'suggestions' && document.getElementById('searchInput') === document.activeElement) {
            target.style.display = target.children.length > 0 ? '' : 'none';
        }
    });

    fitMediumTitle();
});

window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
    document.documentElement.setAttribute('data-bs-theme', e.matches ? 'dark' : 'light');
});

function closeListModal(event) {
    if (event.target.id === 'listModalOverlay') {
        event.target.style.display = 'none';
    }
}

function togglePdfFullscreen() {
    const wrapper = document.getElementById('pdfViewerWrapper');
    if (!document.fullscreenElement && !document.webkitFullscreenElement) {
        if (wrapper.requestFullscreen) {
            wrapper.requestFullscreen();
        } else if (wrapper.webkitRequestFullscreen) {
            wrapper.webkitRequestFullscreen();
        }
    } else {
        if (document.exitFullscreen) {
            document.exitFullscreen();
        } else if (document.webkitExitFullscreen) {
            document.webkitExitFullscreen();
        }
    }
}

function updateFullscreenIcon() {
    const icon = document.getElementById('pdfFullscreenIcon');
    if (!icon) return;
    const isFullscreen = document.fullscreenElement || document.webkitFullscreenElement;
    icon.classList.toggle('fa-compress', !!isFullscreen);
    icon.classList.toggle('fa-expand', !isFullscreen);
}

document.addEventListener('fullscreenchange', updateFullscreenIcon);
document.addEventListener('webkitfullscreenchange', updateFullscreenIcon);

function fitMediumTitle() {
    const title = document.getElementById('medium-title');
    if (!title || window.matchMedia('(min-width: 992px)').matches) return;

    title.style.fontSize = '';

    const style = window.getComputedStyle(title);
    const lineHeight = parseFloat(style.lineHeight);
    const maxHeight = lineHeight * 2 + 1;

    if (title.scrollHeight <= maxHeight) return;

    let lo = 10, hi = parseFloat(style.fontSize);
    while (hi - lo > 0.5) {
        const mid = (lo + hi) / 2;
        title.style.fontSize = mid + 'px';
        if (title.scrollHeight <= maxHeight) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    title.style.fontSize = lo + 'px';
}

window.addEventListener('resize', fitMediumTitle);

// Cross-document view transition: mark the clicked thumbnail so it morphs into the player.
// Uses event delegation so it works for HTMX-loaded cards too.
document.addEventListener('click', function (e) {
    const link = e.target.closest('a[href^="/m/"]');
    if (!link || e.defaultPrevented) return;
    // hx-mediumcard cards use .thumbnail-container; search cards use .search-card-thumbnail
    const thumb = link.querySelector('.thumbnail-container, .search-card-thumbnail');
    if (thumb) {
        thumb.style.viewTransitionName = 'medium-player';
    }
});

// --- studio.html: global upload state ---
window.isUploading = false;

// --- upload.html: upload progress ---
document.addEventListener('DOMContentLoaded', function () {
    var uploadForm = document.getElementById('form');
    if (uploadForm) {
        htmx.on('#form', 'htmx:xhr:progress', function (evt) {
            htmx.find('#progress').setAttribute(
                'value',
                (evt.detail.loaded / evt.detail.total) * 100
            );
        });
    }
});

function beginupload() {
    var submitbutton = document.getElementById('submitbutton');
    if (submitbutton) {
        submitbutton.disabled = true;
        submitbutton.style.display = 'none';
    }
    var progress = document.getElementById('progress');
    if (progress) progress.style.display = 'inline';
    return true;
}

// --- hx-studio-upload.html: upload progress (studio tab) ---
document.body.addEventListener('htmx:afterSettle', function (e) {
    var uploadForm = document.getElementById('upload-form');
    if (!uploadForm) return;
    if (uploadForm._studioUploadBound) return;
    uploadForm._studioUploadBound = true;

    htmx.on('#upload-form', 'htmx:xhr:progress', function (evt) {
        htmx.find('#upload-progress').setAttribute(
            'value',
            (evt.detail.loaded / evt.detail.total) * 100
        );
    });

    htmx.on('#upload-form', 'htmx:afterRequest', function () {
        window.isUploading = false;
    });
});

function beginuploadStudio() {
    window.isUploading = true;
    var btn = document.getElementById('upload-submitbutton');
    if (btn) { btn.disabled = true; btn.style.display = 'none'; }
    var prog = document.getElementById('upload-progress');
    if (prog) prog.style.display = 'inline';
    return true;
}

// --- search.html: filter chips and debounced search ---
document.addEventListener('DOMContentLoaded', function () {
    document.querySelectorAll('.filter-chips').forEach(function (group) {
        group.querySelectorAll('.filter-chip').forEach(function (chip) {
            chip.addEventListener('click', function () {
                group.querySelectorAll('.filter-chip').forEach(function (c) { c.classList.remove('active'); });
                chip.classList.add('active');

                var hiddenInput = group.nextElementSibling;
                if (hiddenInput) {
                    hiddenInput.value = chip.getAttribute('data-value');
                }

                var searchInput = document.getElementById('searchMainInput');
                if (searchInput && searchInput.value.trim().length > 0) {
                    htmx.trigger('#searchForm', 'submit');
                }
            });
        });
    });

    var searchMainInput = document.getElementById('searchMainInput');
    if (searchMainInput) {
        searchMainInput.addEventListener('keydown', function (e) {
            if (e.key === 'Enter') {
                e.preventDefault();
                htmx.trigger('#searchForm', 'submit');
            }
        });

        var debounceTimer;
        searchMainInput.addEventListener('input', function () {
            clearTimeout(debounceTimer);
            debounceTimer = setTimeout(function () {
                if (searchMainInput.value.trim().length > 0) {
                    htmx.trigger('#searchForm', 'submit');
                }
            }, 400);
        });

        if (searchMainInput.value.trim().length > 0) {
            setTimeout(function () { htmx.trigger('#searchForm', 'submit'); }, 100);
        }

        searchMainInput.focus();
    }
});

// --- medium.html: description Quill viewer, comment editor, initCommentDeltas ---
function initCommentDeltas(container) {
    container.querySelectorAll('[data-comment-delta]').forEach(function (el) {
        if (el.dataset.commentInit) return;
        el.dataset.commentInit = '1';
        try {
            var delta = JSON.parse(el.getAttribute('data-comment-delta'));
            var q = new Quill(el, {
                theme: 'snow',
                readOnly: true,
                modules: { toolbar: false },
            });
            q.setContents(delta);
        } catch (e) {
            console.error('Failed to render comment delta', e);
        }
    });
}

document.addEventListener('DOMContentLoaded', function () {
    // Description viewer
    var descViewEl = document.getElementById('medium_description_view');
    if (descViewEl && typeof hljs !== 'undefined') {
        var quillView = new Quill('#medium_description_view', {
            theme: 'snow',
            readOnly: true,
            modules: { toolbar: false, syntax: true },
        });

        quillView.setText('Loading description...');

        quillView.on('text-change', function () {
            document.querySelectorAll('pre code').forEach(function (block) {
                hljs.highlightBlock(block);
            });
        });

        var wrapper = document.getElementById('description_wrapper');
        var toggle = document.getElementById('description_toggle');
        var fade = document.getElementById('description_fade');
        var descView = document.getElementById('medium_description_view');

        function checkOverflow() {
            var contentHeight = descView.scrollHeight;
            var collapsedMax = 6 * 1.5 * 16;
            if (contentHeight > collapsedMax + 10) {
                toggle.classList.remove('d-none');
                fade.style.display = '';
            } else {
                toggle.classList.add('d-none');
                fade.style.display = 'none';
                wrapper.classList.remove('description-collapsed');
            }
        }

        toggle.addEventListener('click', function () {
            var isCollapsed = wrapper.classList.contains('description-collapsed');
            if (isCollapsed) {
                wrapper.classList.remove('description-collapsed');
                fade.style.display = 'none';
                toggle.textContent = 'Show less';
            } else {
                wrapper.classList.add('description-collapsed');
                fade.style.display = '';
                toggle.textContent = 'Show more';
            }
        });

        var pageConfig = document.getElementById('medium-page-config');
        var mediumId = pageConfig ? pageConfig.dataset.mediumId : null;
        if (mediumId) {
            fetch('/m/' + mediumId + '/description.json')
                .then(function (response) {
                    if (!response.ok) throw new Error('Failed to fetch description.json');
                    return response.json();
                })
                .then(function (data) {
                    quillView.setContents(data);
                    setTimeout(checkOverflow, 100);
                })
                .catch(function (err) {
                    console.error(err);
                    quillView.setText('Failed to load description.');
                    setTimeout(checkOverflow, 100);
                });
        }
    }

    // Comment editor
    var commentEditorEl = document.getElementById('comment_editor');
    if (commentEditorEl) {
        var commentQuill = new Quill('#comment_editor', {
            theme: 'snow',
            placeholder: 'Write a comment...',
            modules: {
                toolbar: [
                    ['bold', 'italic', 'underline', 'strike'],
                    ['link', 'code-block'],
                    [{ list: 'ordered' }, { list: 'bullet' }],
                ],
            },
        });

        var submitBtn = document.getElementById('comment_submit_btn');
        submitBtn.addEventListener('click', function () {
            var text = commentQuill.getText().trim();
            if (text.length === 0) return;

            var delta = JSON.stringify(commentQuill.getContents());
            var formData = new FormData();
            formData.append('text', delta);

            submitBtn.disabled = true;
            var pageConfig = document.getElementById('medium-page-config');
            var mediumId = pageConfig ? pageConfig.dataset.mediumId : null;
            if (!mediumId) return;

            fetch('/hx/comments/' + mediumId + '/add', {
                method: 'POST',
                body: new URLSearchParams(formData),
            })
                .then(function (response) {
                    if (!response.ok) throw new Error('Failed to post comment');
                    return response.text();
                })
                .then(function (html) {
                    var commentsList = document.getElementById('comments-list');
                    commentsList.insertAdjacentHTML('afterbegin', html);
                    initCommentDeltas(commentsList);
                    commentQuill.setText('');
                    submitBtn.disabled = false;
                })
                .catch(function (err) {
                    console.error(err);
                    submitBtn.disabled = false;
                });
        });
    }
});

// HTMX: init comment deltas after comments-list is swapped
document.body.addEventListener('htmx:afterSettle', function (e) {
    var target = e.detail.target;
    if (target && target.id === 'comments-list') {
        initCommentDeltas(target);
    }
    // Also init for newly swapped comment items (pagination outerHTML swaps)
    if (target && target.classList && target.classList.contains('hx-placeholder')) {
        var commentsList = document.getElementById('comments-list');
        if (commentsList) initCommentDeltas(commentsList);
    }
});

// --- hx-listmodal.html: TomSelect for list visibility ---
document.body.addEventListener('htmx:afterSettle', function (e) {
    var visEl = document.getElementById('listmodal-visibility');
    var groupEl = document.getElementById('listmodal-group-select');
    if (!visEl || !groupEl || typeof TomSelect === 'undefined') return;
    if (visEl.tomselect) visEl.tomselect.destroy();
    if (groupEl.tomselect) groupEl.tomselect.destroy();
    var visTomSelect = new TomSelect(visEl, { create: false, onFocus: function() { this.clear(true); } });
    var groupTomSelect = new TomSelect(groupEl, { create: false, allowEmptyOption: true, onFocus: function() { this.clear(true); } });
    groupTomSelect.wrapper.style.width = 'auto';
    function updateGroupVisibility() {
        groupTomSelect.wrapper.style.display = visTomSelect.getValue() === 'restricted' ? '' : 'none';
    }
    updateGroupVisibility();
    visEl.addEventListener('change', updateGroupVisibility);
});

// --- hx-settings-diagnostics.html: codec support detection ---
document.body.addEventListener('htmx:afterSettle', function (e) {
    if (!document.getElementById('codec-av1')) return;
    function checkMediaSource(codec) {
        if (typeof MediaSource === 'undefined') return false;
        try { return MediaSource.isTypeSupported(codec); } catch (e) { return false; }
    }
    function setResult(id, supported) {
        var el = document.getElementById(id);
        if (!el) return;
        if (supported) {
            el.innerHTML = '<i class="fa-solid fa-check" style="color:var(--bs-success);"></i>';
        } else {
            el.innerHTML = '<i class="fa-solid fa-xmark" style="color:var(--bs-danger);"></i>';
        }
    }
    setResult('codec-av1', checkMediaSource('video/webm; codecs="av01.0.05M.08"'));
    setResult('codec-vp9', checkMediaSource('video/webm; codecs="vp9"'));
    setResult('codec-h265', checkMediaSource('video/mp4; codecs="hvc1.1.6.L93.B0"'));
    setResult('codec-opus', checkMediaSource('audio/webm; codecs="opus"'));
});

// --- concept.html: Quill description editor ---
document.addEventListener('DOMContentLoaded', function () {
    var conceptDescEl = document.getElementById('medium_description_editor');
    var publishForm = document.getElementById('publish-form');
    if (conceptDescEl && publishForm) {
        var conceptQuill = new Quill('#medium_description_editor', {
            theme: 'snow'
        });

        publishForm.addEventListener('submit', function () {
            var descriptionInput = document.getElementById('medium_description');
            var delta = conceptQuill.getContents();
            descriptionInput.value = JSON.stringify(delta);
            console.log('Submitting content:', descriptionInput.value);
        });

        var hljsScript = document.getElementById('hljs-script');
        if (hljsScript) {
            hljsScript.addEventListener('load', function () {
                if (typeof hljs !== 'undefined') {
                    hljs.highlightAll();
                }
            });
        }
    }
});

// --- concept.html: TomSelect for visibility and group selector ---
document.addEventListener('DOMContentLoaded', function () {
    var visSelect = document.getElementById('medium_visibility');
    var groupSelect = document.getElementById('medium_restricted_group');
    var publishForm = document.getElementById('publish-form');
    if (!visSelect || !groupSelect || !publishForm) return;
    // Avoid re-init if already done (studio-edit also uses these ids)
    if (visSelect.tomselect) return;
    var visTomSelect = new TomSelect('#medium_visibility', { create: false, onFocus: function() { this.clear(true); } });
    var groupTomSelect = new TomSelect('#medium_restricted_group', { create: false, allowEmptyOption: true, onFocus: function() { this.clear(true); } });
    var noGroupsHint = document.getElementById('no-groups-hint');

    function updateGroupVisibility() {
        var wrapper = groupTomSelect.wrapper;
        if (visTomSelect.getValue() === 'restricted') {
            wrapper.style.display = '';
            if (noGroupsHint) noGroupsHint.style.display = '';
        } else {
            wrapper.style.display = 'none';
            if (noGroupsHint) noGroupsHint.style.display = 'none';
        }
    }
    updateGroupVisibility();
    visSelect.addEventListener('change', updateGroupVisibility);
});

// --- studio-edit.html: description Quill editor ---
document.addEventListener('DOMContentLoaded', function () {
    var editDescEl = document.getElementById('medium_description_editor');
    var editForm = document.getElementById('edit-form');
    if (!editDescEl || !editForm) return;

    var editPageConfig = document.getElementById('edit-page-config');
    var mediumId = editPageConfig ? editPageConfig.dataset.mediumId : null;

    var quill = new Quill('#medium_description_editor', {
        theme: 'snow'
    });

    if (mediumId) {
        fetch('/m/' + mediumId + '/description.json')
            .then(function (response) {
                if (!response.ok) throw new Error('Failed to load description');
                return response.json();
            })
            .then(function (data) {
                quill.setContents(data);
            })
            .catch(function (err) {
                console.error('Could not load existing description:', err);
            });
    }

    editForm.addEventListener('submit', function () {
        var descriptionInput = document.getElementById('medium_description');
        var delta = quill.getContents();
        descriptionInput.value = JSON.stringify(delta);
    });
});

// --- studio-edit.html: TomSelect for visibility and group selector ---
document.addEventListener('DOMContentLoaded', function () {
    var visSelect = document.getElementById('medium_visibility');
    var groupSelect = document.getElementById('medium_restricted_group');
    var editForm = document.getElementById('edit-form');
    if (!visSelect || !groupSelect || !editForm) return;
    if (visSelect.tomselect) return;

    var editPageConfig = document.getElementById('edit-page-config');
    var currentGroup = editPageConfig ? editPageConfig.dataset.restrictedGroup : '';

    var visTomSelect = new TomSelect('#medium_visibility', { create: false, onFocus: function() { this.clear(true); } });
    var groupTomSelect = new TomSelect('#medium_restricted_group', { create: false, allowEmptyOption: true, onFocus: function() { this.clear(true); } });
    var noGroupsHint = document.getElementById('no-groups-hint');

    if (currentGroup) {
        groupTomSelect.setValue(currentGroup);
    }

    function updateGroupVisibility() {
        var wrapper = groupTomSelect.wrapper;
        if (visTomSelect.getValue() === 'restricted') {
            wrapper.style.display = '';
            if (noGroupsHint) noGroupsHint.style.display = '';
        } else {
            wrapper.style.display = 'none';
            if (noGroupsHint) noGroupsHint.style.display = 'none';
        }
    }
    updateGroupVisibility();
    visSelect.addEventListener('change', updateGroupVisibility);
});

// --- studio-edit.html: chapters editor ---
document.addEventListener('DOMContentLoaded', function () {
    var chaptersEditor = document.getElementById('chapters-editor');
    if (!chaptersEditor) return;

    var editPageConfig = document.getElementById('edit-page-config');
    var mediumId = editPageConfig ? editPageConfig.dataset.mediumId : null;
    var chapters = [];

    function renderChapters() {
        var tbody = document.getElementById('chapters-tbody');
        var noMsg = document.getElementById('no-chapters-msg');
        tbody.innerHTML = '';

        if (chapters.length === 0) {
            noMsg.style.display = '';
            document.getElementById('chapters-table').style.display = 'none';
            return;
        }
        noMsg.style.display = 'none';
        document.getElementById('chapters-table').style.display = '';

        chapters.forEach(function (ch, idx) {
            var tr = document.createElement('tr');

            var tdStart = document.createElement('td');
            var inputStart = document.createElement('input');
            inputStart.type = 'text';
            inputStart.className = 'form-control form-control-sm';
            inputStart.value = ch.start;
            inputStart.placeholder = '00:00:00';
            tdStart.appendChild(inputStart);

            var tdTitle = document.createElement('td');
            var inputTitle = document.createElement('input');
            inputTitle.type = 'text';
            inputTitle.className = 'form-control form-control-sm';
            inputTitle.value = ch.title;
            tdTitle.appendChild(inputTitle);

            var tdActions = document.createElement('td');
            var btnDelete = document.createElement('button');
            btnDelete.type = 'button';
            btnDelete.className = 'btn btn-sm btn-outline-danger';
            btnDelete.title = 'Remove chapter';
            btnDelete.innerHTML = '<i class="fa-solid fa-trash text-danger"></i>';
            (function (removeIdx) {
                btnDelete.addEventListener('click', function () {
                    chapters = getChaptersFromDOM();
                    chapters.splice(removeIdx, 1);
                    renderChapters();
                });
            })(idx);
            tdActions.appendChild(btnDelete);

            tr.appendChild(tdStart);
            tr.appendChild(tdTitle);
            tr.appendChild(tdActions);
            tbody.appendChild(tr);
        });
    }

    function getChaptersFromDOM() {
        var rows = document.getElementById('chapters-tbody').querySelectorAll('tr');
        var result = [];
        rows.forEach(function (row) {
            var inputs = row.querySelectorAll('input');
            if (inputs.length >= 2) {
                result.push({
                    start: inputs[0].value.trim(),
                    title: inputs[1].value.trim()
                });
            }
        });
        return result;
    }

    if (mediumId) {
        fetch('/studio/edit/' + mediumId + '/chapters.json')
            .then(function (r) { return r.json(); })
            .then(function (data) {
                chapters = data;
                renderChapters();
            })
            .catch(function (err) { console.error('Failed to load chapters:', err); });
    }

    document.getElementById('add-chapter-btn').addEventListener('click', function () {
        chapters = getChaptersFromDOM();
        chapters.push({ start: '00:00:00.000', title: '' });
        renderChapters();
        var lastRow = document.getElementById('chapters-tbody').lastElementChild;
        if (lastRow) {
            var titleInput = lastRow.querySelectorAll('input')[1];
            if (titleInput) titleInput.focus();
        }
    });

    document.getElementById('save-chapters-btn').addEventListener('click', function () {
        var btn = this;
        btn.disabled = true;
        var chaptersToSave = getChaptersFromDOM().filter(function (ch) {
            return ch.title !== '';
        });

        fetch('/studio/edit/' + mediumId + '/chapters', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(chaptersToSave)
        })
            .then(function (r) { return r.json(); })
            .then(function (data) {
                btn.disabled = false;
                var statusEl = document.getElementById('chapters-status');
                if (data.ok) {
                    statusEl.innerHTML = '<span class="text-success"><i class="fa-solid fa-check"></i> Chapters saved successfully!</span>';
                    chapters = chaptersToSave;
                } else {
                    statusEl.innerHTML = '<span class="text-danger"><i class="fa-solid fa-xmark"></i> Failed to save chapters.</span>';
                }
                setTimeout(function () { statusEl.innerHTML = ''; }, 4000);
            })
            .catch(function (err) {
                console.error(err);
                btn.disabled = false;
                document.getElementById('chapters-status').innerHTML = '<span class="text-danger"><i class="fa-solid fa-xmark"></i> Failed to save chapters.</span>';
            });
    });
});

// --- studio-edit.html: subtitles editor ---
document.addEventListener('DOMContentLoaded', function () {
    var subtitlesEditor = document.getElementById('subtitles-editor');
    if (!subtitlesEditor) return;

    var editPageConfig = document.getElementById('edit-page-config');
    var mediumId = editPageConfig ? editPageConfig.dataset.mediumId : null;
    var subtitles = [];

    function renderSubtitles() {
        var tbody = document.getElementById('subtitles-tbody');
        var noMsg = document.getElementById('no-subtitles-msg');
        tbody.innerHTML = '';

        if (subtitles.length === 0) {
            noMsg.style.display = '';
            document.getElementById('subtitles-table').style.display = 'none';
            return;
        }
        noMsg.style.display = 'none';
        document.getElementById('subtitles-table').style.display = '';

        subtitles.forEach(function (sub) {
            var tr = document.createElement('tr');

            var tdLabel = document.createElement('td');
            tdLabel.textContent = sub.label;
            tr.appendChild(tdLabel);

            var tdActions = document.createElement('td');
            var btnDelete = document.createElement('button');
            btnDelete.type = 'button';
            btnDelete.className = 'btn btn-sm btn-outline-danger';
            btnDelete.title = 'Remove subtitle';
            btnDelete.innerHTML = '<i class="fa-solid fa-trash text-danger"></i>';
            (function (label) {
                btnDelete.addEventListener('click', function () {
                    if (!confirm('Delete subtitle track "' + label + '"?')) return;
                    fetch('/studio/edit/' + mediumId + '/subtitles/delete', {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ label: label })
                    })
                        .then(function (r) { return r.json(); })
                        .then(function (data) {
                            if (data.ok) {
                                subtitles = subtitles.filter(function (s) { return s.label !== label; });
                                renderSubtitles();
                                showSubStatus('success', 'Subtitle track deleted.');
                            } else {
                                showSubStatus('danger', data.error || 'Failed to delete.');
                            }
                        })
                        .catch(function () { showSubStatus('danger', 'Failed to delete subtitle.'); });
                });
            })(sub.label);
            tdActions.appendChild(btnDelete);
            tr.appendChild(tdActions);

            tbody.appendChild(tr);
        });
    }

    function showSubStatus(type, msg) {
        var el = document.getElementById('subtitles-status');
        var icon = type === 'success' ? 'fa-check' : 'fa-xmark';
        el.innerHTML = '<span class="text-' + type + '"><i class="fa-solid ' + icon + '"></i> ' + msg + '</span>';
        setTimeout(function () { el.innerHTML = ''; }, 4000);
    }

    if (mediumId) {
        fetch('/studio/edit/' + mediumId + '/subtitles.json')
            .then(function (r) { return r.json(); })
            .then(function (data) {
                subtitles = data;
                renderSubtitles();
            })
            .catch(function (err) { console.error('Failed to load subtitles:', err); });
    }

    document.getElementById('upload-subtitle-btn').addEventListener('click', function () {
        var btn = this;
        var labelInput = document.getElementById('subtitle-label-input');
        var fileInput = document.getElementById('subtitle-file-input');
        var label = labelInput.value.trim();

        if (!label) {
            showSubStatus('danger', 'Please enter a label.');
            return;
        }
        if (!fileInput.files || fileInput.files.length === 0) {
            showSubStatus('danger', 'Please select a VTT file.');
            return;
        }

        var formData = new FormData();
        formData.append('label', label);
        formData.append('file', fileInput.files[0]);

        btn.disabled = true;
        fetch('/studio/edit/' + mediumId + '/subtitles/add', {
            method: 'POST',
            body: formData
        })
            .then(function (r) { return r.json(); })
            .then(function (data) {
                btn.disabled = false;
                if (data.ok) {
                    var exists = subtitles.some(function (s) { return s.label === label; });
                    if (!exists) {
                        subtitles.push({ label: label });
                    }
                    renderSubtitles();
                    labelInput.value = '';
                    fileInput.value = '';
                    showSubStatus('success', 'Subtitle track uploaded!');
                } else {
                    showSubStatus('danger', data.error || 'Upload failed.');
                }
            })
            .catch(function () {
                btn.disabled = false;
                showSubStatus('danger', 'Upload failed.');
            });
    });
});
