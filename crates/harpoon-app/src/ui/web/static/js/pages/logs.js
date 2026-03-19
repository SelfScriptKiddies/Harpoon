window.Pages = window.Pages || {};

Pages.logs = {
  paused: false,
  levelFilter: '',
  searchText: '',

  render() {
    const el = document.getElementById('page-logs');
    let events = App.data.events || [];

    // Classify events as log-like entries
    const logs = events.map(e => ({
      time: e.timestamp_ms,
      level: this.classifyLevel(e.kind),
      component: this.classifyComponent(e.kind),
      message: `[${e.kind}] ${e.detail}`,
      kind: e.kind,
    }));

    let filtered = logs;
    if (this.levelFilter) {
      filtered = filtered.filter(l => l.level === this.levelFilter);
    }
    if (this.searchText) {
      const q = this.searchText.toLowerCase();
      filtered = filtered.filter(l => l.message.toLowerCase().includes(q));
    }

    el.innerHTML = `
      <div style="display:flex;align-items:center;gap:12px;margin-bottom:16px;flex-wrap:wrap;">
        <div class="section-title" style="margin-bottom:0;">Logs <span class="count">${filtered.length}</span></div>
        <select id="logs-level" style="padding:6px 10px;background:var(--bg-1);border:1px solid var(--bg-4);border-radius:var(--radius-sm);color:var(--text);font-size:12px;">
          <option value="">All levels</option>
          <option value="error" ${this.levelFilter === 'error' ? 'selected' : ''}>Error</option>
          <option value="warn" ${this.levelFilter === 'warn' ? 'selected' : ''}>Warning</option>
          <option value="info" ${this.levelFilter === 'info' ? 'selected' : ''}>Info</option>
        </select>
        <input class="search-input" id="logs-search" placeholder="Search logs..."
               value="${App.esc(this.searchText)}" style="flex:1;min-width:200px;">
        <button class="btn btn-sm" onclick="Pages.logs.togglePause()">
          ${this.paused ? 'Resume' : 'Pause'}
        </button>
      </div>

      <div class="code-block" style="max-height:calc(100vh - 160px);overflow-y:auto;font-size:12px;line-height:1.6;">
        ${filtered.length === 0 ? '<div class="empty"><div class="empty-text">No log entries</div></div>' :
          filtered.slice().reverse().map(l => {
            const color = l.level === 'error' ? 'var(--err)' : l.level === 'warn' ? 'var(--warn)' : 'var(--text-3)';
            return `<div style="padding:2px 0;border-bottom:1px solid var(--bg-4);">` +
              `<span style="color:var(--text-4);">${App.fmtTime(l.time)}</span> ` +
              `<span style="color:${color};font-weight:500;">${l.level.toUpperCase().padEnd(5)}</span> ` +
              `<span style="color:var(--text-3);">[${App.esc(l.component)}]</span> ` +
              `<span style="color:var(--text-2);">${App.esc(l.message)}</span>` +
              `</div>`;
          }).join('')}
      </div>
    `;

    document.getElementById('logs-level')?.addEventListener('change', (ev) => {
      this.levelFilter = ev.target.value;
      if (!this.paused) this.render();
    });
    document.getElementById('logs-search')?.addEventListener('input', (ev) => {
      this.searchText = ev.target.value;
      if (!this.paused) this.render();
    });
  },

  classifyLevel(kind) {
    if (kind.includes('error') || kind.includes('drop')) return 'error';
    if (kind.includes('timeout') || kind.includes('closed')) return 'warn';
    return 'info';
  },

  classifyComponent(kind) {
    if (kind.includes('tcp')) return 'tcp';
    if (kind.includes('udp')) return 'udp';
    if (kind.includes('filter')) return 'filter';
    if (kind.includes('export')) return 'export';
    if (kind.includes('rule')) return 'engine';
    return 'system';
  },

  togglePause() {
    this.paused = !this.paused;
    this.render();
  }
};
