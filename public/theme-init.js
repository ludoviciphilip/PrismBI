// Aplica o tema escuro antes do React montar (evita flash branco).
// Arquivo externo — e não <script> inline — para funcionar com a CSP
// `script-src 'self'` definida em src-tauri/tauri.conf.json.
(function () {
  try {
    var mode = localStorage.getItem('insighthub_theme_mode') || 'light';
    var isDark =
      mode === 'dark' ||
      (mode === 'system' && window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches);
    if (isDark) {
      document.documentElement.classList.add('dark');
      document.documentElement.style.colorScheme = 'dark';
    }
  } catch (e) {}
})();
