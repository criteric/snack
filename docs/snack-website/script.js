const menu = document.querySelector('.menu');
const nav = document.querySelector('#navlinks');
menu.addEventListener('click', () => { const open = nav.classList.toggle('open'); menu.setAttribute('aria-expanded', String(open)); });
nav.querySelectorAll('a').forEach(a => a.addEventListener('click', () => { nav.classList.remove('open'); menu.setAttribute('aria-expanded', 'false'); }));
document.querySelector('#year').textContent = new Date().getFullYear();
const toast = document.querySelector('.toast');
document.querySelectorAll('[data-copy]').forEach(button => button.addEventListener('click', async () => {
  try { await navigator.clipboard.writeText(button.dataset.copy); toast.textContent = 'Copied to clipboard'; }
  catch { toast.textContent = 'Copy: ' + button.dataset.copy; }
  toast.classList.add('show'); setTimeout(() => toast.classList.remove('show'), 2400);
}));
