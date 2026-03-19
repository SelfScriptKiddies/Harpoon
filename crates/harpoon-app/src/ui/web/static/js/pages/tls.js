window.Pages = window.Pages || {};

Pages.tls = {
  render() {
    const el = document.getElementById('page-tls');
    const rulesFull = App.data.rulesFull || [];
    const tlsRules = rulesFull.filter(r => r.tls && r.tls.mode !== 'disabled');
    const stats = App.data.stats || [];

    el.innerHTML = `
      <div class="section-title">TLS</div>

      <div class="cards">
        <div class="card">
          <div class="card-label">TLS Rules</div>
          <div class="card-value">${tlsRules.length}</div>
        </div>
        <div class="card">
          <div class="card-label">MITM Rules</div>
          <div class="card-value">${tlsRules.filter(r => r.tls.mode === 'mitm').length}</div>
        </div>
        <div class="card">
          <div class="card-label">Terminate Rules</div>
          <div class="card-value">${tlsRules.filter(r => r.tls.mode === 'terminate').length}</div>
        </div>
        <div class="card">
          <div class="card-label">Passthrough Rules</div>
          <div class="card-value">${tlsRules.filter(r => r.tls.mode === 'passthrough').length}</div>
        </div>
      </div>

      ${tlsRules.length === 0 ? `
        <div class="empty" style="margin-top:32px;">
          <div class="empty-icon">🔒</div>
          <div class="empty-text">No TLS rules configured</div>
          <div style="color:var(--text-4);font-size:12px;margin-top:8px;">
            Create a TCP rule with TLS mode set to terminate or mitm
          </div>
        </div>
      ` : `
        <div class="section">
          <div class="section-title">TLS Rules</div>
          <div class="tbl-wrap">
            <table>
              <thead><tr>
                <th>Rule</th><th>Mode</th><th>Listen</th><th>Target</th>
                <th>CA Cert</th><th>CA Key</th><th>Status</th>
              </tr></thead>
              <tbody>
                ${tlsRules.map(r => {
                  const s = stats.find(x => x.rule_name === r.name);
                  return `
                    <tr>
                      <td><strong>${App.esc(r.name)}</strong></td>
                      <td><span class="badge badge-tls">${App.esc(r.tls.mode).toUpperCase()}</span></td>
                      <td class="mono">${App.esc(r.listen)}</td>
                      <td class="mono">${App.esc(r.target)}</td>
                      <td class="mono" style="max-width:200px;overflow:hidden;text-overflow:ellipsis;" title="${App.esc(r.tls.ca_cert)}">${App.esc(r.tls.ca_cert)}</td>
                      <td class="mono" style="max-width:200px;overflow:hidden;text-overflow:ellipsis;" title="${App.esc(r.tls.ca_key)}">${App.esc(r.tls.ca_key)}</td>
                      <td><span class="badge badge-ok">ACTIVE</span></td>
                    </tr>
                  `;
                }).join('')}
              </tbody>
            </table>
          </div>
        </div>

        <div class="section">
          <div class="section-title">TLS Traffic Stats</div>
          <div class="tbl-wrap">
            <table>
              <thead><tr>
                <th>Rule</th><th>TCP Connections</th><th>Bytes C→S</th><th>Bytes S→C</th><th>Filter Hits</th><th>Dropped</th>
              </tr></thead>
              <tbody>
                ${tlsRules.map(r => {
                  const s = stats.find(x => x.rule_name === r.name) || {};
                  return `
                    <tr>
                      <td><strong>${App.esc(r.name)}</strong></td>
                      <td class="mono">${s.active_tcp_connections || 0}</td>
                      <td class="mono">${App.fmtBytes(s.bytes_client_to_server || 0)}</td>
                      <td class="mono">${App.fmtBytes(s.bytes_server_to_client || 0)}</td>
                      <td class="mono">${s.filter_matches || 0}</td>
                      <td class="mono">${s.dropped_packets || 0}</td>
                    </tr>
                  `;
                }).join('')}
              </tbody>
            </table>
          </div>
        </div>
      `}
    `;
  }
};
