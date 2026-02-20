function toggleSidebar() {
    var isMobile = window.matchMedia("(max-width: 1000px)").matches;
    if (isMobile) {
        document.getElementById("sidebar").classList.toggle("sidebar-open");
        document.getElementById("sidebarbackground").classList.toggle("sidebar-open");
    } else {
        document.body.classList.toggle("sidebar-collapsed");
    }
}

document.addEventListener('DOMContentLoaded', () => {
    document.getElementById("sidebarbackground").addEventListener("click", function () {
        if (window.matchMedia("(max-width: 1000px)").matches) {
            document.getElementById("sidebar").classList.remove("sidebar-open");
            this.classList.remove("sidebar-open");
        }
    });

    const searchInput = document.getElementById('searchInput');
    const suggestionsList = document.getElementById('suggestions');

    document.getElementById('suggestions').addEventListener('click', (e) => {
        if (e.target.tagName === 'LI') {
            searchInput.value = e.target.textContent;
            suggestionsList.innerHTML = '';
        }
    });

    searchInput.addEventListener('focus', () => {
        if (suggestionsList.children.length > 0) {
            suggestionsList.style.display = '';
        }
    });

    searchInput.addEventListener('blur', () => {
        setTimeout(() => {
            suggestionsList.style.display = 'none';
        }, 200);
    });

    document.body.addEventListener('htmx:afterSwap', (event) => {
        const target = event.detail.target;
        if (target.id === 'suggestions' && document.getElementById('searchInput') === document.activeElement) {
            target.style.display = target.children.length > 0 ? '' : 'none';
        }
    });
});

window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function() {
    var d = window.matchMedia('(prefers-color-scheme: dark)').matches;
    document.documentElement.setAttribute('data-bs-theme', d ? 'dark' : 'light');
});

function closeListModal(event) {
    if (event.target.id === 'listModalOverlay') {
        event.target.style.display = 'none';
    }
}