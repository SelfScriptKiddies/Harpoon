window.Pages = window.Pages || {};

Pages.traffic = {
  paused: false,
  filter: '',

  render() {
    const el = document.getElementById('page-traffic');
    const events = App.data.events || [];
    const filtered = this.filter
      ? events.filter(e => e.kind.includes(this.filter) || e.detail.includes(this.filter))
      : events;
    const dataEvents = filtered.filter(e =>
      e.kind.includes('data') || e.kind.includes('conn') || e.kind.includes('session')
    );

    el.innerHTML = `
      <div style="display:flex;align-items:center;gap:12px;margin-bottom:16px;flex-wrap:wrap;">
        <div class="section-title" style="margin-bottom:0;">Live Traffic</div>
        <input class="search-input" id="traffic-filter" placeholder="Filter by kind or detail..."
               value="${App.esc(this.filter)}" style="flex:1;min-width:200px;">
        <button class="btn btn-sm" id="traffic-pause-btn" onclick="Pages.traffic.togglePause()">
          ${this.paused ? 'Resume' : 'Pause'}
        </button>
        <button class="btn btn-sm" onclick="Pages.traffic.clear()">Clear</button>
      </div>
      <div class="tbl-wrap" style="max-height:calc(100vh - 160px);overflow-y:auto;">
        <table>
          <thead><tr>
            <th style="width:140px;">Time</th>
            <th style="width:160px;">Kind</th>
            <th>Detail</th>
          </tr></thead>
          <tbody>
            ${dataEvents.length === 0 ? `<tr><td colspan="3"><div class="empty"><div class="empty-text">No traffic events yet</div></div></td></tr>` :
              dataEvents.slice().reverse().map(e => `
                <tr>
                  <td class="mono" style="color:var(--text-3);">${App.fmtTime(e.timestamp_ms)}</td>
                  <td><span class="badge ${this.kindBadge(e.kind)}">${App.esc(e.kind)}</span></td>
                  <td class="mono" style="color:var(--text-2);overflow:hidden;text-overflow:ellipsis;max-width:500px;">${App.esc(e.detail)}</td>
                </tr>
              `).join('')}
          </tbody>
        </table>
      </div>
    `;

    document.getElementById('traffic-filter')?.addEventListener('input', (ev) => {
      this.filter = ev.target.value;
      if (!this.paused) this.render();
    });
  },

  kindBadge(kind) {
    if (kind.includes('drop') || kind.includes('error')) return 'badge-err';
    if (kind.includes('created') || kind.includes('opened') || kind.includes('activated')) return 'badge-ok';
    if (kind.includes('closed') || kind.includes('timeout')) return 'badge-warn';
    return 'badge-info';
  },

  togglePause() {
    this.paused = !this.paused;
    this.render();
  },

  clear() {
    App.data.events = [];
    this.render();
  }
};
