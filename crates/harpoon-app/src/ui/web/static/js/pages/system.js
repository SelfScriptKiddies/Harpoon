window.Pages = window.Pages || {};

Pages.system = {
  render() {
    const el = document.getElementById('page-system');
    const status = App.data.status || {};
    const stats = App.data.stats || [];
    const rulesFull = App.data.rulesFull || [];

    const totalBytes = stats.reduce((a, s) => a + s.bytes_client_to_server + s.bytes_server_to_client, 0);
    const hasTls = rulesFull.some(r => r.tls);
    const hasExporters = rulesFull.some(r => r.exporter);
    const hasFilters = rulesFull.some(r => (r.filters || []).length > 0);

    el.innerHTML = `
      <div class="section-title">System</div>

      <div class="cards">
        <div class="card">
          <div class="card-label">Version</div>
          <div class="card-value" style="font-size:16px;">0.1.0</div>
        </div>
        <div class="card">
          <div class="card-label">Uptime</div>
          <div class="card-value" style="font-size:16px;">${App.fmtUptime(status.uptime_secs || 0)}</div>
        </div>
        <div class="card">
          <div class="card-label">Status</div>
          <div class="card-value ${status.running ? 'ok' : 'err'}" style="font-size:16px;">
            ${status.running ? 'Running' : 'Stopped'}
          </div>
        </div>
        <div class="card">
          <div class="card-label">Total Traffic</div>
          <div class="card-value" style="font-size:16px;">${App.fmtBytes(totalBytes)}</div>
        </div>
        <div class="card">
          <div class="card-label">Active Rules</div>
          <div class="card-value">${status.rules_count || 0}</div>
        </div>
        <div class="card">
          <div class="card-label">Config Path</div>
          <div class="card-value mono" style="font-size:12px;word-break:break-all;">${App.esc(status.config_path || '—')}</div>
        </div>
      </div>

      <div class="section">
        <div class="section-title">Enabled Features</div>
        <div style="display:flex;gap:8px;flex-wrap:wrap;">
          <span class="badge badge-ok">WEB UI</span>
          ${hasTls ? '<span class="badge badge-tls">TLS MITM</span>' : '<span class="badge badge-muted">TLS (no rules)</span>'}
          ${hasExporters ? '<span class="badge badge-export">EXPORTERS</span>' : '<span class="badge badge-muted">EXPORTERS (none)</span>'}
          ${hasFilters ? '<span class="badge badge-info">FILTERS</span>' : '<span class="badge badge-muted">FILTERS (none)</span>'}
        </div>
      </div>

      <div class="section">
        <div class="section-title">Build Information</div>
        <div class="detail">
          <div class="detail-row"><span class="detail-key">Binary</span><span class="detail-val">harpoon</span></div>
          <div class="detail-row"><span class="detail-key">Version</span><span class="detail-val">0.1.0</span></div>
          <div class="detail-row"><span class="detail-key">Platform</span><span class="detail-val">Linux</span></div>
          <div class="detail-row"><span class="detail-key">Runtime</span><span class="detail-val">tokio (multi-threaded)</span></div>
          <div class="detail-row"><span class="detail-key">Core Library</span><span class="detail-val">harpoon-core 0.1.0</span></div>
          <div class="detail-row"><span class="detail-key">Web Framework</span><span class="detail-val">axum</span></div>
        </div>
      </div>

      <div class="section">
        <div class="section-title">Control Plane</div>
        <div class="detail">
          <div class="detail-row"><span class="detail-key">Control Socket</span><span class="detail-val">/tmp/harpoon.sock</span></div>
          <div class="detail-row"><span class="detail-key">Web UI</span><span class="detail-val">Active (this page)</span></div>
          <div class="detail-row"><span class="detail-key">Protocol</span><span class="detail-val">JSON over UDS (length-prefixed) + HTTP REST</span></div>
        </div>
      </div>
    `;
  }
};
