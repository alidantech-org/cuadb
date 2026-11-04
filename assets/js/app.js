// Browser-only behaviors live here. Data requests and pagination will use HTMX.
document.addEventListener('click', (event) => {
  const target = event.target;
  if (!(target instanceof Element)) return;

  const toggle = target.closest('[data-toggle-sidebar]');
  if (!(toggle instanceof HTMLButtonElement)) return;

  const sidebar = document.getElementById('sidebar');
  if (!sidebar) return;

  const collapsed = sidebar.classList.toggle('is-collapsed');
  toggle.setAttribute('aria-expanded', String(!collapsed));
});
