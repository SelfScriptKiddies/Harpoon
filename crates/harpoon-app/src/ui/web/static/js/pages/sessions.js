window.Pages = window.Pages || {};

Pages.sessions = {
  render() {
    const el = document.getElementById('page-sessions');
    const stats = App.data.stats || [];
    const totalTcp = stats.reduce((a, s) => a + s.active_tcp_connections, 0);
    const totalUdp = stats.reduce((a, s) => a + s.active_udp_sessions, 0);
    const rulesFull = App.data.rulesFull || [];

    el.innerHTML = `
      <div class="section-title">Sessions</div>
      <div class="cards">
        <div class="card">
          <div class="card-label">Total TCP Connections</div>
          <div class="card-value">${totalTcp}</div>
        </div>
        <div class="card">
          <div class="card-label">Total UDP Sessions</div>
          <div class="card-value">${totalUdp}</div>
        </div>
        <div class="card">
          <div class="card-label">Rules with Traffic</div>
          <div class="card-value">${stats.filter(s => s.active_tcp_connections > 0 || s.active_udp_sessions > 0).length}</div>
        </div>
      </div>

      <div class="section">
        <div class="section-title">TCP Connections <span class="count">${totalTcp}</span></div>
        <div class="tbl-wrap">
          <table>
            <thead><tr>
              <th>Rule</th><th>Protocol</th><th>Listen</th><th>Target</th>
              <th>Active Conns</th><th>Bytes C→S</th><th>Bytes S→C</th><th>Dropped</th>
            </tr></thead>
            <tbody>
              ${stats.filter(s => {
                const r = rulesFull.find(x => x.name === s.rule_name);
                return !r || r.protocol === 'tcp';
              }).map(s => `
                <tr>
                  <td><strong>${App.esc(s.rule_name)}</strong></td>
                  <td><span class="badge badge-tcp">TCP</span></td>
                  <td class="mono">${this.findListen(s.rule_name)}</td>
                  <td class="mono">${this.findTarget(s.rule_name)}</td>
                  <td class="mono">${s.active_tcp_connections}</td>
                  <td class="mono">${App.fmtBytes(s.bytes_client_to_server)}</td>
                  <td class="mono">${App.fmtBytes(s.bytes_server_to_client)}</td>
                  <td class="mono">${s.dropped_packets}</td>
                </tr>
              `).join('') || '<tr><td colspan="8"><div class="empty"><div class="empty-text">No TCP rules</div></div></td></tr>'}
            </tbody>
          </table>
        </div>
      </div>

      <div class="section">
        <div class="section-title">UDP Sessions <span class="count">${totalUdp}</span></div>
        <div class="tbl-wrap">
          <table>
            <thead><tr>
              <th>Rule</th><th>Protocol</th><th>Listen</th><th>Target</th>
              <th>Active Sess</th><th>Bytes C→S</th><th>Bytes S→C</th>
              <th>Pkts C→S</th><th>Pkts S→C</th><th>Idle Timeout</th><th>Source Mode</th>
            </tr></thead>
            <tbody>
              ${stats.filter(s => {
                const r = rulesFull.find(x => x.name === s.rule_name);
                return r && r.protocol === 'udp';
              }).map(s => {
                const r = rulesFull.find(x => x.name === s.rule_name);
                return `
                  <tr>
                    <td><strong>${App.esc(s.rule_name)}</strong></td>
                    <td><span class="badge badge-udp">UDP</span></td>
                    <td class="mono">${App.esc(r?.listen || '')}</td>
                    <td class="mono">${App.esc(r?.target || '')}</td>
                    <td class="mono">${s.active_udp_sessions}</td>
                    <td class="mono">${App.fmtBytes(s.bytes_client_to_server)}</td>
                    <td class="mono">${App.fmtBytes(s.bytes_server_to_client)}</td>
                    <td class="mono">${s.packets_client_to_server}</td>
                    <td class="mono">${s.packets_server_to_client}</td>
                    <td class="mono">${r?.idle_timeout_secs || 30}s</td>
                    <td><span class="badge ${r?.udp_source_mode === 'preserve' ? 'badge-warn' : 'badge-muted'}">${App.esc(r?.udp_source_mode || 'proxy')}</span></td>
                  </tr>
                `;
              }).join('') || '<tr><td colspan="11"><div class="empty"><div class="empty-text">No UDP rules</div></div></td></tr>'}
            </tbody>
          </table>
        </div>
      </div>
    `;
  },

  findListen(name) {
    const r = App.data.rules.find(x => x.name === name);
    return App.esc(r?.listen || '');
  },

  findTarget(name) {
    const r = App.data.rules.find(x => x.name === name);
    return App.esc(r?.target || '');
  }
};
