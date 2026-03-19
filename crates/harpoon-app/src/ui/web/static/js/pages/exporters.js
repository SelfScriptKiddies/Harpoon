window.Pages = window.Pages || {};

Pages.exporters = {
  render() {
    const el = document.getElementById('page-exporters');
    const rulesFull = App.data.rulesFull || [];
    const stats = App.data.stats || [];
    const rulesWithExp = rulesFull.filter(r => r.exporter);

    el.innerHTML = `
      <div class="section-title">Exporters</div>

      <div class="cards">
        <div class="card">
          <div class="card-label">Configured Exporters</div>
          <div class="card-value">${rulesWithExp.length}</div>
        </div>
        <div class="card">
          <div class="card-label">UDS Exporters</div>
          <div class="card-value">${rulesWithExp.filter(r => r.exporter.kind === 'uds' || r.exporter.kind === 'unix').length}</div>
        </div>
        <div class="card">
          <div class="card-label">TCP Framed Exporters</div>
          <div class="card-value">${rulesWithExp.filter(r => r.exporter.kind === 'tcp' || r.exporter.kind === 'tcp_framed').length}</div>
        </div>
        <div class="card">
          <div class="card-label">Total Export Drops</div>
          <div class="card-value ${stats.reduce((a, s) => a + s.export_drops, 0) > 0 ? 'warn' : ''}">${stats.reduce((a, s) => a + s.export_drops, 0)}</div>
        </div>
      </div>

      ${rulesWithExp.length === 0 ? `
        <div class="empty" style="margin-top:32px;">
          <div class="empty-icon">📤</div>
          <div class="empty-text">No exporters configured</div>
          <div style="color:var(--text-4);font-size:12px;margin-top:8px;">
            Add an exporter to a rule to send events to an external sink (UDS or TCP)
          </div>
        </div>
      ` : `
        <div class="section">
          <div class="section-title">Exporter Bindings</div>
          <div class="tbl-wrap">
            <table>
              <thead><tr>
                <th>Rule</th><th>Type</th><th>Destination</th>
                <th>Export Drops</th><th>Bytes C→S</th><th>Bytes S→C</th>
              </tr></thead>
              <tbody>
                ${rulesWithExp.map(r => {
                  const s = stats.find(x => x.rule_name === r.name) || {};
                  const dest = r.exporter.path || r.exporter.addr || '—';
                  const kindLabel = (r.exporter.kind === 'uds' || r.exporter.kind === 'unix') ? 'UDS' : 'TCP Framed';
                  return `
                    <tr>
                      <td><strong>${App.esc(r.name)}</strong></td>
                      <td><span class="badge badge-export">${kindLabel}</span></td>
                      <td class="mono">${App.esc(dest)}</td>
                      <td class="mono ${(s.export_drops || 0) > 0 ? 'style="color:var(--warn);"' : ''}">${s.export_drops || 0}</td>
                      <td class="mono">${App.fmtBytes(s.bytes_client_to_server || 0)}</td>
                      <td class="mono">${App.fmtBytes(s.bytes_server_to_client || 0)}</td>
                    </tr>
                  `;
                }).join('')}
              </tbody>
            </table>
          </div>
        </div>

        <div class="section">
          <div class="section-title">Export Protocol</div>
          <div class="detail">
            <div class="detail-row"><span class="detail-key">Format</span><span class="detail-val">Length-prefixed binary frames</span></div>
            <div class="detail-row"><span class="detail-key">Frame Header</span><span class="detail-val">4 bytes (u32 BE) length + 1 byte version + 1 byte event kind</span></div>
            <div class="detail-row"><span class="detail-key">Payload</span><span class="detail-val">8 bytes timestamp (ms) + rule name (len-prefixed) + detail (len-prefixed)</span></div>
            <div class="detail-row"><span class="detail-key">Version</span><span class="detail-val">0x01</span></div>
          </div>
        </div>
      `}
    `;
  }
};
