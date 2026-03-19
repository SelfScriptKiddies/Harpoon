<script>
  import { reload } from '../lib/api.js';

  let { data, onNavigate, onCreatePipeline } = $props();

  function fmtBytes(n) { if(!n) return '0 B'; if(n<1024) return n+' B'; if(n<1048576) return (n/1024).toFixed(1)+' KB'; if(n<1073741824) return (n/1048576).toFixed(1)+' MB'; return (n/1073741824).toFixed(2)+' GB'; }
  function fmtUptime(s) { if(!s) return '\u2014'; if(s<60) return s+'s'; if(s<3600) return Math.floor(s/60)+'m '+(s%60)+'s'; return Math.floor(s/3600)+'h '+Math.floor((s%3600)/60)+'m'; }
  function fmtTime(ms) { if(!ms) return '\u2014'; const d=new Date(ms); return d.toLocaleTimeString('en-GB',{hour12:false})+'.'+String(ms%1000).padStart(3,'0'); }

  let status = $derived(data?.status ?? {});
  let stats = $derived(data?.stats ?? []);
  let rules = $derived(data?.rules ?? []);
  let events = $derived(data?.events ?? []);

  let isRunning = $derived(status.running ?? false);
  let uptimeStr = $derived(fmtUptime(status.uptime_secs));

  let totalBytesIn = $derived(stats.reduce((s, r) => s + (r.bytes_client_to_server ?? 0), 0));
  let totalBytesOut = $derived(stats.reduce((s, r) => s + (r.bytes_server_to_client ?? 0), 0));
  let totalTcp = $derived(stats.reduce((s, r) => s + (r.active_tcp_connections ?? 0), 0));
  let totalUdp = $derived(stats.reduce((s, r) => s + (r.active_udp_sessions ?? 0), 0));
  let totalDropped = $derived(stats.reduce((s, r) => s + (r.dropped_packets ?? 0), 0));
  let totalFilterHits = $derived(stats.reduce((s, r) => s + (r.filter_matches ?? 0), 0));

  let recentEvents = $derived(
    [...events].sort((a, b) => (b.timestamp_ms ?? 0) - (a.timestamp_ms ?? 0)).slice(0, 30)
  );

  let reloading = $state(false);

  async function handleReload() {
    reloading = true;
    try { await reload(); } catch { /* ignore */ }
    reloading = false;
  }
</script>

<div class="overview">
  <div class="page-header">
    <h1 class="page-title">Overview</h1>
    <span class="page-subtitle mono">{isRunning ? 'Operational' : 'Offline'}</span>
  </div>

  <!-- Status Cards -->
  <div class="status-grid">
    <div class="card stat-card">
      <div class="stat-label">Status</div>
      <div class="stat-value">
        <span class="status-dot" class:running={isRunning}></span>
        {isRunning ? 'Running' : 'Stopped'}
      </div>
      <div class="stat-sub mono">{uptimeStr} uptime</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Bytes In</div>
      <div class="stat-value mono">{fmtBytes(totalBytesIn)}</div>
      <div class="stat-sub">client &rarr; server</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Bytes Out</div>
      <div class="stat-value mono">{fmtBytes(totalBytesOut)}</div>
      <div class="stat-sub">server &rarr; client</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">TCP Connections</div>
      <div class="stat-value mono">{totalTcp}</div>
      <div class="stat-sub">active</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">UDP Sessions</div>
      <div class="stat-value mono">{totalUdp}</div>
      <div class="stat-sub">active</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Dropped Packets</div>
      <div class="stat-value mono" class:val-warn={totalDropped > 0}>{totalDropped}</div>
      <div class="stat-sub">by filters</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Filter Hits</div>
      <div class="stat-value mono">{totalFilterHits}</div>
      <div class="stat-sub">total matches</div>
    </div>
  </div>

  <!-- Quick Actions -->
  <div class="actions-row">
    <button class="btn btn-accent" onclick={() => onCreatePipeline?.()}>+ Create Pipeline</button>
    <button class="btn" onclick={handleReload} disabled={reloading}>
      {reloading ? 'Reloading...' : 'Reload Config'}
    </button>
    <button class="btn" onclick={() => onNavigate?.('events')}>View Events</button>
  </div>

  <!-- Rules Table -->
  <div class="card table-section">
    <div class="section-title">Rules ({rules.length})</div>
    {#if rules.length === 0}
      <div class="empty-state">No rules configured. Create a pipeline to get started.</div>
    {:else}
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Name</th>
              <th>Protocol</th>
              <th>Listen</th>
              <th>Target</th>
              <th>Filters</th>
              <th>Features</th>
            </tr>
          </thead>
          <tbody>
            {#each rules as rule}
              <tr>
                <td class="mono">{rule.name}</td>
                <td>
                  <span class="badge" class:badge-tcp={rule.protocol === 'tcp'} class:badge-udp={rule.protocol === 'udp'}>
                    {rule.protocol}
                  </span>
                </td>
                <td class="mono">{rule.listen}</td>
                <td class="mono">{rule.target}</td>
                <td>{rule.filters_count ?? 0}</td>
                <td>
                  {#if rule.has_duplicate}<span class="badge badge-warn">dup</span>{/if}
                  {#if rule.has_exporter}<span class="badge badge-ok">export</span>{/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>

  <!-- Per-Rule Stats -->
  {#if stats.length > 0}
    <div class="card table-section">
      <div class="section-title">Per-Rule Traffic</div>
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Rule</th>
              <th>Bytes In</th>
              <th>Bytes Out</th>
              <th>Pkts In</th>
              <th>Pkts Out</th>
              <th>TCP</th>
              <th>UDP</th>
              <th>Dropped</th>
              <th>Filters</th>
              <th>Export Drops</th>
            </tr>
          </thead>
          <tbody>
            {#each stats as s}
              <tr>
                <td class="mono">{s.rule_name}</td>
                <td class="mono">{fmtBytes(s.bytes_client_to_server)}</td>
                <td class="mono">{fmtBytes(s.bytes_server_to_client)}</td>
                <td class="mono">{s.packets_client_to_server ?? 0}</td>
                <td class="mono">{s.packets_server_to_client ?? 0}</td>
                <td class="mono">{s.active_tcp_connections ?? 0}</td>
                <td class="mono">{s.active_udp_sessions ?? 0}</td>
                <td class="mono" class:val-warn={s.dropped_packets > 0}>{s.dropped_packets ?? 0}</td>
                <td class="mono">{s.filter_matches ?? 0}</td>
                <td class="mono" class:val-warn={s.export_drops > 0}>{s.export_drops ?? 0}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  {/if}

  <!-- Recent Events -->
  <div class="card table-section">
    <div class="section-title">Recent Events (last 30)</div>
    {#if recentEvents.length === 0}
      <div class="empty-state">No events recorded yet.</div>
    {:else}
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Time</th>
              <th>Kind</th>
              <th>Detail</th>
            </tr>
          </thead>
          <tbody>
            {#each recentEvents as ev}
              <tr>
                <td class="mono">{fmtTime(ev.timestamp_ms)}</td>
                <td>
                  <span class="badge"
                    class:badge-ok={ev.kind === 'info' || ev.kind === 'start'}
                    class:badge-err={ev.kind === 'error'}
                    class:badge-warn={ev.kind === 'warning' || ev.kind === 'drop'}
                    class:badge-tcp={ev.kind === 'connection'}
                  >{ev.kind}</span>
                </td>
                <td class="detail-cell">{ev.detail ?? ''}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</div>

<style>
  .overview { display: flex; flex-direction: column; gap: 20px; }

  .page-header { display: flex; align-items: baseline; gap: 12px; }
  .page-title { font-size: 22px; font-weight: 700; color: var(--text); }
  .page-subtitle { font-size: 12px; color: var(--ok); }

  .status-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 12px;
  }

  .stat-card { display: flex; flex-direction: column; gap: 4px; }
  .stat-label { font-size: 11px; color: var(--text-3); text-transform: uppercase; letter-spacing: .5px; }
  .stat-value { font-size: 20px; font-weight: 700; color: var(--text); display: flex; align-items: center; gap: 8px; }
  .stat-sub { font-size: 11px; color: var(--text-4); }

  .status-dot {
    width: 8px; height: 8px; border-radius: 50%; background: var(--err); flex-shrink: 0;
  }
  .status-dot.running {
    background: var(--ok); box-shadow: 0 0 6px rgba(112, 217, 138, 0.5);
  }

  .val-warn { color: var(--warn); }

  .actions-row { display: flex; gap: 8px; flex-wrap: wrap; }

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

  .detail-cell { white-space: normal; word-break: break-all; max-width: 400px; }
  .empty-state { color: var(--text-4); font-size: 13px; padding: 20px 0; text-align: center; }
</style>
