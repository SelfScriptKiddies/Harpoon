<script>
  let { data } = $props();

  function fmtBytes(n) { if(!n) return '0 B'; if(n<1024) return n+' B'; if(n<1048576) return (n/1024).toFixed(1)+' KB'; if(n<1073741824) return (n/1048576).toFixed(1)+' MB'; return (n/1073741824).toFixed(2)+' GB'; }
  function fmtUptime(s) { if(!s) return '\u2014'; if(s<60) return s+'s'; if(s<3600) return Math.floor(s/60)+'m '+(s%60)+'s'; return Math.floor(s/3600)+'h '+Math.floor((s%3600)/60)+'m'; }
  function fmtTime(ms) { if(!ms) return '\u2014'; const d=new Date(ms); return d.toLocaleTimeString('en-GB',{hour12:false})+'.'+String(ms%1000).padStart(3,'0'); }

  let stats = $derived(data?.stats ?? []);
  let rulesFull = $derived(data?.rulesFull ?? []);

  let totalTcp = $derived(stats.reduce((s, r) => s + (r.active_tcp_connections ?? 0), 0));
  let totalUdp = $derived(stats.reduce((s, r) => s + (r.active_udp_sessions ?? 0), 0));

  let tcpStats = $derived(stats.filter(s => (s.active_tcp_connections ?? 0) > 0 || findRule(s.rule_name)?.protocol === 'tcp'));
  let udpStats = $derived(stats.filter(s => (s.active_udp_sessions ?? 0) > 0 || findRule(s.rule_name)?.protocol === 'udp'));

  function findRule(name) {
    return rulesFull.find(r => r.name === name);
  }
</script>

<div class="sessions-page">
  <div class="page-header">
    <h1 class="page-title">Sessions</h1>
  </div>

  <!-- Summary Cards -->
  <div class="summary-grid">
    <div class="card stat-card">
      <div class="stat-label">Total TCP Connections</div>
      <div class="stat-value">
        <span class="badge badge-tcp">TCP</span>
        <span class="mono">{totalTcp}</span>
      </div>
      <div class="stat-sub">across {tcpStats.length} rule{tcpStats.length !== 1 ? 's' : ''}</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Total UDP Sessions</div>
      <div class="stat-value">
        <span class="badge badge-udp">UDP</span>
        <span class="mono">{totalUdp}</span>
      </div>
      <div class="stat-sub">across {udpStats.length} rule{udpStats.length !== 1 ? 's' : ''}</div>
    </div>
  </div>

  <!-- TCP Table -->
  <div class="card table-section">
    <div class="section-title">TCP Connections</div>
    {#if tcpStats.length === 0}
      <div class="empty-state">No TCP rules or active TCP connections.</div>
    {:else}
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Rule</th>
              <th>Listen</th>
              <th>Target</th>
              <th>Active</th>
              <th>Bytes In</th>
              <th>Bytes Out</th>
              <th>Pkts In</th>
              <th>Pkts Out</th>
              <th>Dropped</th>
              <th>Filter Hits</th>
            </tr>
          </thead>
          <tbody>
            {#each tcpStats as s}
              {@const rule = findRule(s.rule_name)}
              <tr>
                <td class="mono">{s.rule_name}</td>
                <td class="mono">{rule?.listen ?? '\u2014'}</td>
                <td class="mono">{rule?.target ?? '\u2014'}</td>
                <td class="mono val-highlight">{s.active_tcp_connections ?? 0}</td>
                <td class="mono">{fmtBytes(s.bytes_client_to_server)}</td>
                <td class="mono">{fmtBytes(s.bytes_server_to_client)}</td>
                <td class="mono">{s.packets_client_to_server ?? 0}</td>
                <td class="mono">{s.packets_server_to_client ?? 0}</td>
                <td class="mono" class:val-warn={s.dropped_packets > 0}>{s.dropped_packets ?? 0}</td>
                <td class="mono">{s.filter_matches ?? 0}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>

  <!-- UDP Table -->
  <div class="card table-section">
    <div class="section-title">UDP Sessions</div>
    {#if udpStats.length === 0}
      <div class="empty-state">No UDP rules or active UDP sessions.</div>
    {:else}
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Rule</th>
              <th>Listen</th>
              <th>Target</th>
              <th>Active</th>
              <th>Bytes In</th>
              <th>Bytes Out</th>
              <th>Pkts In</th>
              <th>Pkts Out</th>
              <th>Idle Timeout</th>
              <th>Source Mode</th>
              <th>Dropped</th>
            </tr>
          </thead>
          <tbody>
            {#each udpStats as s}
              {@const rule = findRule(s.rule_name)}
              <tr>
                <td class="mono">{s.rule_name}</td>
                <td class="mono">{rule?.listen ?? '\u2014'}</td>
                <td class="mono">{rule?.target ?? '\u2014'}</td>
                <td class="mono val-highlight">{s.active_udp_sessions ?? 0}</td>
                <td class="mono">{fmtBytes(s.bytes_client_to_server)}</td>
                <td class="mono">{fmtBytes(s.bytes_server_to_client)}</td>
                <td class="mono">{s.packets_client_to_server ?? 0}</td>
                <td class="mono">{s.packets_server_to_client ?? 0}</td>
                <td class="mono">{rule?.idle_timeout_secs ?? 30}s</td>
                <td class="mono">{rule?.udp_source_mode ?? 'connected'}</td>
                <td class="mono" class:val-warn={s.dropped_packets > 0}>{s.dropped_packets ?? 0}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</div>

<style>
  .sessions-page { display: flex; flex-direction: column; gap: 20px; }

  .page-header { display: flex; align-items: baseline; gap: 12px; }
  .page-title { font-size: 22px; font-weight: 700; color: var(--text); }

  .summary-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; max-width: 500px; }

  .stat-card { display: flex; flex-direction: column; gap: 4px; }
  .stat-label { font-size: 11px; color: var(--text-3); text-transform: uppercase; letter-spacing: .5px; }
  .stat-value { font-size: 22px; font-weight: 700; color: var(--text); display: flex; align-items: center; gap: 8px; }
  .stat-sub { font-size: 11px; color: var(--text-4); }

  .table-section { overflow: hidden; }
  .table-wrap { overflow-x: auto; }

  table { width: 100%; border-collapse: collapse; font-size: 12px; }
  thead th {
    text-align: left; padding: 8px 10px; font-size: 10px; font-weight: 600;
    color: var(--text-3); text-transform: uppercase; letter-spacing: .5px;
    border-bottom: 1px solid var(--bg-4); white-space: nowrap;
  }
  tbody td {
    padding: 7px 10px; border-bottom: 1px solid var(--bg-3); color: var(--text-2);
    white-space: nowrap;
  }
  tbody tr:hover { background: var(--bg-3); }
  tbody tr:last-child td { border-bottom: none; }

  .val-highlight { color: var(--accent); font-weight: 600; }
  .val-warn { color: var(--warn); }

  .empty-state { color: var(--text-4); font-size: 13px; padding: 20px 0; text-align: center; }
</style>
