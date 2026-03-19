/*  Harpoon Web Admin Panel – core application JS
 *  -----------------------------------------------
 *  Provides the global App object that every page module relies on:
 *  auth, navigation, API helpers, data refresh loop, toasts, modals,
 *  and a handful of formatting utilities.
 */

window.Pages = {};

const App = {
  token: localStorage.getItem('harpoon_token') || '',
  data: { status: null, stats: [], rules: [], rulesFull: [], events: [] },

  /* ── API helper ─────────────────────────────────────────────── */

  async api(path, opts = {}) {
    opts.headers = opts.headers || {};
    if (App.token) {
      opts.headers['Authorization'] = 'Bearer ' + App.token;
    }
    const res = await fetch(path, opts);
    if (res.status === 401) {
      App.showLogin();
    }
    return res;
  },

  /* ── Auth ────────────────────────────────────────────────────── */

  showLogin() {
    App.token = '';
    localStorage.removeItem('harpoon_token');
    document.getElementById('login-page').style.display = '';
    document.getElementById('app').style.display = 'none';
  },

  showApp() {
    document.getElementById('login-page').style.display = 'none';
    document.getElementById('app').style.display = '';
  },

  async doLogin() {
    const user = document.getElementById('login-user').value.trim();
    const pass = document.getElementById('login-pass').value;
    const errEl = document.getElementById('login-error');
    errEl.textContent = '';

    try {
      const res = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username: user, password: pass }),
      });

      if (res.ok) {
        const body = await res.json();
        App.token = body.token;
        localStorage.setItem('harpoon_token', App.token);
        App.showApp();
        await App.refreshAll();
      } else {
        errEl.textContent = 'Invalid credentials';
      }
    } catch (e) {
      errEl.textContent = 'Login failed: ' + e.message;
    }
  },

  doLogout() {
    App.showLogin();
  },

  /* ── Navigation ─────────────────────────────────────────────── */

  nav(el) {
    document.querySelectorAll('.nav-item').forEach(n => n.classList.remove('active'));
    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));

    el.classList.add('active');
    const page = el.getAttribute('data-page');
    const pageEl = document.getElementById('page-' + page);
    if (pageEl) pageEl.classList.add('active');

    if (Pages[page] && typeof Pages[page].render === 'function') {
      Pages[page].render();
    }
  },

  navTo(page) {
    const el = document.querySelector('.nav-item[data-page="' + page + '"]');
    if (el) App.nav(el);
  },

  /* ── Toast notifications ────────────────────────────────────── */

  toast(msg, type = 'ok') {
    const area = document.getElementById('toast-area');
    const el = document.createElement('div');
    el.className = 'toast ' + type;
    el.textContent = msg;
    area.appendChild(el);
    setTimeout(() => { el.remove(); }, 3000);
  },

  /* ── Data refresh ───────────────────────────────────────────── */

  async refreshAll() {
    try {
      const [statusRes, statsRes, rulesRes, rulesFullRes, eventsRes] =
        await Promise.all([
          App.api('/api/status'),
          App.api('/api/stats'),
          App.api('/api/rules'),
          App.api('/api/rules/full'),
          App.api('/api/events'),
        ]);

      if (statusRes.ok)    App.data.status    = await statusRes.json();
      if (statsRes.ok)      App.data.stats      = await statsRes.json();
      if (rulesRes.ok)      App.data.rules      = await rulesRes.json();
      if (rulesFullRes.ok)  App.data.rulesFull  = await rulesFullRes.json();
      if (eventsRes.ok)     App.data.events     = await eventsRes.json();
    } catch (e) {
      // Network error – leave stale data in place; topbar will show err
      App.data.status = null;
    }

    App.renderTopbar();

    const active = document.querySelector('.nav-item.active');
    if (active) {
      const page = active.getAttribute('data-page');
      if (Pages[page] && typeof Pages[page].render === 'function') {
        Pages[page].render();
      }
    }
  },

  /* ── Topbar ─────────────────────────────────────────────────── */

  renderTopbar() {
    const dot   = document.getElementById('status-dot');
    const label = document.getElementById('status-label');

    if (App.data.status && App.data.status.running) {
      dot.className   = 'status-dot ok';
      label.textContent = 'Running';
    } else {
      dot.className   = 'status-dot err';
      label.textContent = App.data.status ? 'Stopped' : 'Unreachable';
    }

    const rulesCount = document.getElementById('chip-rules');
    const tcpCount   = document.getElementById('chip-tcp');
    const udpCount   = document.getElementById('chip-udp');
    const uptimeEl   = document.getElementById('chip-uptime');

    if (rulesCount) rulesCount.textContent = App.data.rules.length;

    let totalTcp = 0;
    let totalUdp = 0;
    for (const s of App.data.stats) {
      totalTcp += s.active_tcp_connections || 0;
      totalUdp += s.active_udp_sessions   || 0;
    }
    if (tcpCount) tcpCount.textContent = totalTcp;
    if (udpCount) udpCount.textContent = totalUdp;

    if (uptimeEl && App.data.status) {
      uptimeEl.textContent = App.fmtUptime(App.data.status.uptime_secs || 0);
    }
  },

  /* ── Modal management ───────────────────────────────────────── */

  showModal(html) {
    const overlay = document.querySelector('.modal-overlay');
    overlay.innerHTML = '<div class="modal">' + html + '</div>';
    overlay.style.display = 'flex';
    overlay.onclick = function (e) {
      if (e.target === overlay) App.closeModal();
    };
  },

  closeModal() {
    const overlay = document.querySelector('.modal-overlay');
    overlay.style.display = 'none';
    overlay.innerHTML = '';
    overlay.onclick = null;
  },

  /* ── Utility functions ──────────────────────────────────────── */

  fmtBytes(n) {
    if (n == null) return '0 B';
    if (n < 1024) return n + ' B';
    if (n < 1024 * 1024) return (n / 1024).toFixed(1) + ' KB';
    if (n < 1024 * 1024 * 1024) return (n / (1024 * 1024)).toFixed(1) + ' MB';
    return (n / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
  },

  fmtUptime(s) {
    s = Math.floor(s);
    if (s < 60) return s + 's';
    if (s < 3600) return Math.floor(s / 60) + 'm ' + (s % 60) + 's';
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    return h + 'h ' + m + 'm';
  },

  fmtTime(ms) {
    const d = new Date(ms);
    const hh = String(d.getHours()).padStart(2, '0');
    const mm = String(d.getMinutes()).padStart(2, '0');
    const ss = String(d.getSeconds()).padStart(2, '0');
    const ml = String(d.getMilliseconds()).padStart(3, '0');
    return hh + ':' + mm + ':' + ss + '.' + ml;
  },

  esc(s) {
    const el = document.createElement('span');
    el.textContent = s;
    return el.innerHTML;
  },

  /* ── Init ───────────────────────────────────────────────────── */

  async init() {
    // Wire up Enter key on password field
    const passEl = document.getElementById('login-pass');
    if (passEl) {
      passEl.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') App.doLogin();
      });
    }

    // Check existing token
    if (App.token) {
      try {
        const res = await App.api('/api/status');
        if (res.ok) {
          App.showApp();
          await App.refreshAll();
        } else {
          App.showLogin();
        }
      } catch (e) {
        App.showLogin();
      }
    } else {
      App.showLogin();
    }

    // Periodic refresh
    setInterval(() => {
      if (App.token) App.refreshAll();
    }, 3000);
  },
};

/* ── Global function bindings for HTML onclick handlers ────────── */

window.doLogin       = () => App.doLogin();
window.doLogout      = () => App.doLogout();
window.doReload      = async () => {
  await App.api('/api/reload', { method: 'POST' });
  App.toast('Config reloaded');
  App.refreshAll();
};
window.doStop        = async () => {
  if (!confirm('Shutdown?')) return;
  await App.api('/api/stop', { method: 'POST' });
  App.toast('Shutdown initiated');
};
window.navClick      = (el) => App.nav(el);
window.openCreateRule = () => Pages.rules.openForm();

document.addEventListener('DOMContentLoaded', () => App.init());
