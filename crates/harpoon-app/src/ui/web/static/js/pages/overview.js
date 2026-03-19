/*  Harpoon – Overview page (main dashboard)
 *  ------------------------------------------
 *  Renders a dense operational snapshot: status cards, health panel,
 *  quick actions, rules summary, per-rule traffic stats, recent events.
 */

window.Pages = window.Pages || {};

Pages.overview = {

  render() {
    const el = document.getElementById('page-overview');
    const status = App.data.status || {};
    const stats  = App.data.stats  || [];
    const rules  = App.data.rules  || [];
    const events = App.data.events || [];

    /* ── aggregate stats ─────────────────────────────────────── */

    let bytesIn  = 0, bytesOut = 0, pktsIn = 0, pktsOut = 0;
    let tcp = 0, udp = 0, dropped = 0, filterHits = 0, exportDrops = 0;

    for (const s of stats) {
      bytesIn     += s.bytes_client_to_server    || 0;
      bytesOut    += s.bytes_server_to_client     || 0;
      pktsIn      += s.packets_client_to_server   || 0;
      pktsOut     += s.packets_server_to_client    || 0;
      tcp         += s.active_tcp_connections      || 0;
      udp         += s.active_udp_sessions         || 0;
      dropped     += s.dropped_packets             || 0;
      filterHits  += s.filter_matches              || 0;
      exportDrops += s.export_drops                || 0;
    }

    const running   = !!status.running;
    const uptime    = App.fmtUptime(status.uptime_secs || 0);
    const exporters = rules.filter(r => r.has_exporter).length;

    /* ── build HTML ──────────────────────────────────────────── */

    el.innerHTML =

      /* 1. Status Cards */
      `<div class="cards">` +
        card('Status', running ? 'Running' : 'Stopped', running ? 'ok' : 'err', 'uptime ' + uptime) +
        card('Total Bytes In',  App.fmtBytes(bytesIn))  +
        card('Total Bytes Out', App.fmtBytes(bytesOut)) +
        card('Packets In',  pktsIn.toLocaleString())  +
        card('Packets Out', pktsOut.toLocaleString()) +
        card('TCP Connections', tcp.toLocaleString()) +
        card('UDP Sessions',    udp.toLocaleString()) +
        card('Dropped', dropped.toLocaleString(), dropped > 0 ? 'warn' : '') +
        card('Filter Hits', filterHits.toLocaleString()) +
        card('Export Drops', exportDrops.toLocaleString(), exportDrops > 0 ? 'warn' : '') +
      `</div>` +

      /* 2. Health Panel */
      `<div class="section">
        <div class="detail" style="padding:14px 20px;">
          <div style="display:flex;align-items:center;gap:24px;flex-wrap:wrap;">
            <span style="display:inline-flex;align-items:center;gap:6px;">
              <strong style="color:var(--text-3);font-size:12px;text-transform:uppercase;letter-spacing:.04em;">Engine</strong>
              <span class="badge ${running ? 'badge-ok' : 'badge-err'}">${running ? 'Running' : 'Stopped'}</span>
            </span>
            <span style="display:inline-flex;align-items:center;gap:6px;">
              <strong style="color:var(--text-3);font-size:12px;text-transform:uppercase;letter-spacing:.04em;">Rules</strong>
              <span class="badge ${rules.length > 0 ? 'badge-ok' : 'badge-warn'}">${rules.length} active</span>
            </span>
            <span style="display:inline-flex;align-items:center;gap:6px;">
              <strong style="color:var(--text-3);font-size:12px;text-transform:uppercase;letter-spacing:.04em;">Exporters</strong>
              <span class="badge badge-export">${exporters} configured</span>
            </span>
            <span style="display:inline-flex;align-items:center;gap:6px;">
              <strong style="color:var(--text-3);font-size:12px;text-transform:uppercase;letter-spacing:.04em;">Config</strong>
              <code class="mono" style="font-size:12px;color:var(--text-2);">${App.esc(status.config_path || '--')}</code>
            </span>
          </div>
        </div>
      </div>` +

      /* 3. Quick Actions */
      `<div class="section" style="display:flex;gap:10px;flex-wrap:wrap;">
        <button class="btn btn-accent" onclick="openCreateRule()">+ Create Rule</button>
        <button class="btn" onclick="doReload()">Reload Config</button>
        <button class="btn" onclick="App.navTo('events')">View Events</button>
        <button class="btn" onclick="App.navTo('traffic')">View Traffic</button>
      </div>` +

      /* 4. Rules Summary Table */
      `<div class="section">
        <div class="section-title">Rules <span class="count">${rules.length}</span></div>` +
        (rules.length === 0
          ? `<div class="detail"><div class="empty"><div class="empty-text">No rules configured</div></div></div>`
          : `<div class="tbl-wrap"><table>
              <thead><tr>
                <th>Name</th><th>Proto</th><th>Listen</th><th>Target</th>
                <th>Filters</th><th>Duplicate</th><th>Exporter</th>
              </tr></thead>
              <tbody>${rules.map(r => `
                <tr style="cursor:pointer;" onclick="App.navTo('rules')">
                  <td><strong>${App.esc(r.name)}</strong></td>
                  <td><span class="badge badge-${r.protocol === 'tcp' ? 'tcp' : r.protocol === 'udp' ? 'udp' : 'muted'}">${App.esc(r.protocol)}</span></td>
                  <td class="mono">${App.esc(r.listen)}</td>
                  <td class="mono">${App.esc(r.target)}</td>
                  <td class="mono">${r.filters_count || 0}</td>
                  <td>${r.has_duplicate ? '<span class="badge badge-ok">yes</span>' : '<span class="badge badge-muted">no</span>'}</td>
                  <td>${r.has_exporter  ? '<span class="badge badge-export">yes</span>' : '<span class="badge badge-muted">no</span>'}</td>
                </tr>`).join('')}
              </tbody>
            </table></div>`) +
      `</div>` +

      /* 5. Per-Rule Stats Table */
      `<div class="section">
        <div class="section-title">Traffic Statistics</div>` +
        (stats.length === 0
          ? `<div class="detail"><div class="empty"><div class="empty-text">No traffic statistics available</div></div></div>`
          : `<div class="tbl-wrap"><table>
              <thead><tr>
                <th>Rule</th>
                <th>Bytes C&#8594;S</th><th>Bytes S&#8594;C</th>
                <th>Pkts C&#8594;S</th><th>Pkts S&#8594;C</th>
                <th>TCP</th><th>UDP</th>
                <th>Dropped</th><th>Filter Hits</th><th>Export Drops</th>
              </tr></thead>
              <tbody>${stats.map(s => `
                <tr>
                  <td><strong>${App.esc(s.rule_name)}</strong></td>
                  <td class="mono">${App.fmtBytes(s.bytes_client_to_server)}</td>
                  <td class="mono">${App.fmtBytes(s.bytes_server_to_client)}</td>
                  <td class="mono">${(s.packets_client_to_server || 0).toLocaleString()}</td>
                  <td class="mono">${(s.packets_server_to_client || 0).toLocaleString()}</td>
                  <td class="mono">${(s.active_tcp_connections || 0).toLocaleString()}</td>
                  <td class="mono">${(s.active_udp_sessions || 0).toLocaleString()}</td>
                  <td class="mono">${(s.dropped_packets || 0).toLocaleString()}</td>
                  <td class="mono">${(s.filter_matches || 0).toLocaleString()}</td>
                  <td class="mono">${(s.export_drops || 0).toLocaleString()}</td>
                </tr>`).join('')}
              </tbody>
            </table></div>`) +
      `</div>` +

      /* 6. Recent Events */
      `<div class="section">
        <div class="section-title">Recent Events <span class="count">${events.length}</span></div>
        <div class="detail" style="padding:0;max-height:300px;overflow-y:auto;">` +
          (events.length === 0
            ? `<div class="empty"><div class="empty-text">No events recorded</div></div>`
            : events.slice().reverse().slice(0, 30).map(e => {
                const kindLower = (e.kind || '').toLowerCase();
                let kindColor = 'var(--info)';
                if (kindLower.includes('error') || kindLower.includes('drop')) {
                  kindColor = 'var(--err)';
                } else if (kindLower.includes('created') || kindLower.includes('activated')) {
                  kindColor = 'var(--ok)';
                }
                return `<div class="event-row">
                  <span class="event-time">${App.fmtTime(e.timestamp_ms)}</span>
                  <span class="event-kind" style="color:${kindColor};">${App.esc(e.kind)}</span>
                  <span class="event-detail">${App.esc(e.detail)}</span>
                </div>`;
              }).join('')) +
        `</div>
      </div>`;
  }
};

/* ── helper: build a stat card ────────────────────────────────── */

function card(label, value, cls, sub) {
  return `<div class="card">
    <div class="card-label">${App.esc(label)}</div>
    <div class="card-value${cls ? ' ' + cls : ''}">${App.esc(String(value))}</div>
    ${sub ? '<div class="card-sub">' + App.esc(sub) + '</div>' : ''}
  </div>`;
}
