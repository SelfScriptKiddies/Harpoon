window.Pages = window.Pages || {};

Pages.events = {
  paused: false,
  filterKind: '',
  filterText: '',

  render() {
    const el = document.getElementById('page-events');
    let events = App.data.events || [];

    if (this.filterKind) {
      events = events.filter(e => e.kind === this.filterKind);
    }
    if (this.filterText) {
      const q = this.filterText.toLowerCase();
      events = events.filter(e => e.kind.toLowerCase().includes(q) || e.detail.toLowerCase().includes(q));
    }

    const kinds = [...new Set((App.data.events || []).map(e => e.kind))].sort();

    el.innerHTML = `
      <div style="display:flex;align-items:center;gap:12px;margin-bottom:16px;flex-wrap:wrap;">
        <div class="section-title" style="margin-bottom:0;">Events <span class="count">${events.length}</span></div>
        <select id="events-kind-filter" style="padding:6px 10px;background:var(--bg-1);border:1px solid var(--bg-4);border-radius:var(--radius-sm);color:var(--text);font-size:12px;">
          <option value="">All kinds</option>
          ${kinds.map(k => `<option value="${App.esc(k)}" ${k === this.filterKind ? 'selected' : ''}>${App.esc(k)}</option>`).join('')}
        </select>
        <input class="search-input" id="events-text-filter" placeholder="Search events..."
               value="${App.esc(this.filterText)}" style="flex:1;min-width:200px;">
        <button class="btn btn-sm" onclick="Pages.events.togglePause()">
          ${this.paused ? 'Resume' : 'Pause'}
        </button>
        <button class="btn btn-sm" onclick="Pages.events.clearEvents()">Clear</button>
      </div>

      <div class="tbl-wrap" style="max-height:calc(100vh - 160px);overflow-y:auto;">
        <table>
          <thead><tr>
            <th style="width:140px;">Time</th>
            <th style="width:180px;">Kind</th>
            <th>Detail</th>
          </tr></thead>
          <tbody>
            ${events.length === 0 ? `<tr><td colspan="3"><div class="empty"><div class="empty-text">No events</div></div></td></tr>` :
              events.slice().reverse().map(e => `
                <tr>
                  <td class="mono" style="color:var(--text-3);">${App.fmtTime(e.timestamp_ms)}</td>
                  <td><span class="badge ${this.kindBadge(e.kind)}">${App.esc(e.kind)}</span></td>
                  <td class="mono" style="color:var(--text-2);">${App.esc(e.detail)}</td>
                </tr>
              `).join('')}
          </tbody>
        </table>
      </div>
    `;

    document.getElementById('events-kind-filter')?.addEventListener('change', (ev) => {
      this.filterKind = ev.target.value;
      if (!this.paused) this.render();
    });
    document.getElementById('events-text-filter')?.addEventListener('input', (ev) => {
      this.filterText = ev.target.value;
      if (!this.paused) this.render();
    });
  },

  kindBadge(kind) {
    if (kind.includes('drop') || kind.includes('error')) return 'badge-err';
    if (kind.includes('created') || kind.includes('opened') || kind.includes('activated')) return 'badge-ok';
    if (kind.includes('closed') || kind.includes('timeout')) return 'badge-warn';
    if (kind.includes('match')) return 'badge-info';
    return 'badge-muted';
  },

  togglePause() {
    this.paused = !this.paused;
    this.render();
  },

  clearEvents() {
    App.data.events = [];
    this.render();
  }
};
