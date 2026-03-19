window.Pages = window.Pages || {};

Pages.config = {
  render() {
    const el = document.getElementById('page-config');
    const status = App.data.status || {};
    const rulesFull = App.data.rulesFull || [];

    el.innerHTML = `
      <div class="section-title">Configuration</div>

      <div class="detail">
        <h3>Loaded Config</h3>
        <div class="detail-row"><span class="detail-key">Path</span><span class="detail-val">${App.esc(status.config_path || '—')}</span></div>
        <div class="detail-row"><span class="detail-key">Rules</span><span class="detail-val">${rulesFull.length}</span></div>
      </div>

      <div style="display:flex;gap:8px;margin-bottom:24px;flex-wrap:wrap;">
        <button class="btn btn-accent" onclick="doReload()">Reload Config</button>
        <button class="btn" onclick="Pages.config.downloadConfig()">Download TOML</button>
        <button class="btn" onclick="Pages.config.showRawToml()">View Raw TOML</button>
      </div>

      <div class="section">
        <div class="section-title">Effective Rules</div>
        <div class="tbl-wrap">
          <table>
            <thead><tr>
              <th>Name</th><th>Protocol</th><th>Listen</th><th>Target</th>
              <th>TLS</th><th>Filters</th><th>Duplicate</th><th>Exporter</th><th>UDP Mode</th><th>Timeout</th>
            </tr></thead>
            <tbody>
              ${rulesFull.length === 0 ?
                '<tr><td colspan="10"><div class="empty"><div class="empty-text">No rules configured</div></div></td></tr>' :
                rulesFull.map(r => `
                  <tr>
                    <td><strong>${App.esc(r.name)}</strong></td>
                    <td><span class="badge badge-${r.protocol}">${r.protocol.toUpperCase()}</span></td>
                    <td class="mono">${App.esc(r.listen)}</td>
                    <td class="mono">${App.esc(r.target)}</td>
                    <td>${r.tls ? `<span class="badge badge-tls">${App.esc(r.tls.mode)}</span>` : '—'}</td>
                    <td>${(r.filters || []).length}</td>
                    <td class="mono">${r.duplicate || '—'}</td>
                    <td>${r.exporter ? `<span class="badge badge-export">${App.esc(r.exporter.kind)}</span>` : '—'}</td>
                    <td>${r.protocol === 'udp' ? App.esc(r.udp_source_mode || 'proxy') : '—'}</td>
                    <td class="mono">${r.idle_timeout_secs || 30}s</td>
                  </tr>
                `).join('')}
            </tbody>
          </table>
        </div>
      </div>

      <div class="section" id="config-toml-section" style="display:none;">
        <div style="display:flex;align-items:center;gap:8px;margin-bottom:12px;">
          <div class="section-title" style="margin-bottom:0;">Raw TOML</div>
          <button class="btn btn-sm" onclick="Pages.config.copyToml()">Copy</button>
          <button class="btn btn-sm" onclick="document.getElementById('config-toml-section').style.display='none'">Close</button>
        </div>
        <div class="code-block" id="config-toml-content" style="max-height:400px;overflow-y:auto;"></div>
      </div>
    `;
  },

  async showRawToml() {
    try {
      const r = await App.api('/api/config/toml');
      const data = await r.json();
      const section = document.getElementById('config-toml-section');
      const content = document.getElementById('config-toml-content');
      if (section && content) {
        content.textContent = data.toml || 'Unable to load';
        section.style.display = 'block';
      }
    } catch (e) {
      App.toast('Failed to load config', 'err');
    }
  },

  copyToml() {
    const content = document.getElementById('config-toml-content');
    if (content) {
      navigator.clipboard.writeText(content.textContent);
      App.toast('Config copied to clipboard');
    }
  },

  async downloadConfig() {
    try {
      const r = await App.api('/api/config/toml');
      const data = await r.json();
      const blob = new Blob([data.toml || ''], { type: 'application/toml' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'harpoon-config.toml';
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      App.toast('Failed to download config', 'err');
    }
  }
};
