<script>
  let { data } = $props();

  function fmtBytes(n) { if(!n) return '0 B'; if(n<1024) return n+' B'; if(n<1048576) return (n/1024).toFixed(1)+' KB'; return (n/1048576).toFixed(1)+' MB'; }
  function fmtTime(ms) { if(!ms) return '\u2014'; const d=new Date(ms); return d.toLocaleTimeString('en-GB',{hour12:false})+'.'+String(ms%1000).padStart(3,'0'); }

  let rulesFull = $derived(data?.rulesFull ?? []);
  let stats = $derived(data?.stats ?? []);

  let tlsRules = $derived(rulesFull.filter(r => r.tls));

  let mitmCount = $derived(tlsRules.filter(r => r.tls.mode === 'mitm').length);
  let terminateCount = $derived(tlsRules.filter(r => r.tls.mode === 'terminate').length);
  let passthroughCount = $derived(tlsRules.filter(r => r.tls.mode === 'passthrough').length);

  let firstTlsRule = $derived(tlsRules.length > 0 ? tlsRules[0] : null);

  let tlsStats = $derived(
    stats.filter(s => tlsRules.some(r => r.name === s.rule_name))
  );

  function tlsModeBadgeClass(mode) {
    if (mode === 'mitm') return 'badge-accent';
    if (mode === 'terminate') return 'badge-info';
    return 'badge-muted';
  }

  function truncPath(path, maxLen = 40) {
    if (!path) return '\u2014';
    if (path.length <= maxLen) return path;
    return '\u2026' + path.slice(-(maxLen - 1));
  }

  function findStats(ruleName) {
    return stats.find(s => s.rule_name === ruleName);
  }
</script>

<div class="tls-page">
  <div class="page-header">
    <h1 class="page-title">TLS</h1>
    <span class="page-subtitle mono">{tlsRules.length} TLS rule{tlsRules.length !== 1 ? 's' : ''}</span>
  </div>

  <!-- Status Cards -->
  <div class="cards-grid">
    <div class="card stat-card">
      <div class="stat-label">TLS Rules</div>
      <div class="stat-value mono">{tlsRules.length}</div>
      <div class="stat-sub">configured</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">MITM</div>
      <div class="stat-value mono">{mitmCount}</div>
      <div class="stat-sub">intercept + re-encrypt</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Terminate</div>
      <div class="stat-value mono">{terminateCount}</div>
      <div class="stat-sub">decrypt at proxy</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Passthrough</div>
      <div class="stat-value mono">{passthroughCount}</div>
      <div class="stat-sub">forward encrypted</div>
    </div>
  </div>

  {#if tlsRules.length > 0}
    <!-- CA Status -->
    {#if firstTlsRule.tls.ca_cert || firstTlsRule.tls.ca_key}
      <div class="card">
        <div class="section-title">CA Status</div>
        <div class="ca-grid">
          <div class="ca-item">
            <span class="ca-label">CA Certificate</span>
            <span class="ca-value mono">{firstTlsRule.tls.ca_cert ?? 'not set'}</span>
          </div>
          <div class="ca-item">
            <span class="ca-label">CA Private Key</span>
            <span class="ca-value mono">{firstTlsRule.tls.ca_key ?? 'not set'}</span>
          </div>
        </div>
      </div>
    {/if}

    <!-- TLS Rules Table -->
    <div class="card table-section">
      <div class="section-title">TLS Rules</div>
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Name</th>
              <th>Mode</th>
              <th>Listen</th>
              <th>Target</th>
              <th>CA Cert</th>
            </tr>
          </thead>
          <tbody>
            {#each tlsRules as rule}
              <tr>
                <td class="mono">{rule.name}</td>
                <td>
                  <span class="badge {tlsModeBadgeClass(rule.tls.mode)}">
                    {(rule.tls.mode ?? 'terminate').toUpperCase()}
                  </span>
                </td>
                <td class="mono">{rule.listen}</td>
                <td class="mono">{rule.target}</td>
                <td class="mono path-cell" title={rule.tls.ca_cert ?? ''}>{truncPath(rule.tls.ca_cert)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>

    <!-- TLS Traffic Stats -->
    {#if tlsStats.length > 0}
      <div class="card table-section">
        <div class="section-title">TLS Traffic Stats</div>
        <div class="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Rule</th>
                <th>TCP Conns</th>
                <th>Bytes C2S</th>
                <th>Bytes S2C</th>
                <th>Filter Hits</th>
                <th>Dropped</th>
              </tr>
            </thead>
            <tbody>
              {#each tlsStats as s}
                <tr>
                  <td class="mono">{s.rule_name}</td>
                  <td class="mono">{s.active_tcp_connections ?? 0}</td>
                  <td class="mono">{fmtBytes(s.bytes_client_to_server)}</td>
                  <td class="mono">{fmtBytes(s.bytes_server_to_client)}</td>
                  <td class="mono">{s.filter_matches ?? 0}</td>
                  <td class="mono" class:val-warn={s.dropped_packets > 0}>{s.dropped_packets ?? 0}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    {/if}
  {:else}
    <!-- Empty State -->
    <div class="card empty-card">
      <div class="empty-icon">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
          <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
        </svg>
      </div>
      <div class="empty-title">No TLS rules configured</div>
      <div class="empty-hint">Create a TCP rule with a TLS mode (mitm, terminate, or passthrough) to enable TLS interception and inspection.</div>
    </div>
  {/if}
</div>

<style>
  .tls-page { display: flex; flex-direction: column; gap: 20px; }

  .page-header { display: flex; align-items: baseline; gap: 12px; }
  .page-title { font-size: 22px; font-weight: 700; color: var(--text); }
  .page-subtitle { font-size: 12px; color: var(--text-3); }

  .cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: 12px;
  }

  .stat-card { display: flex; flex-direction: column; gap: 4px; }
  .stat-label { font-size: 11px; color: var(--text-3); text-transform: uppercase; letter-spacing: .5px; }
  .stat-value { font-size: 20px; font-weight: 700; color: var(--text); }
  .stat-sub { font-size: 11px; color: var(--text-4); }

  .ca-grid { display: flex; flex-direction: column; gap: 10px; }
  .ca-item { display: flex; flex-direction: column; gap: 3px; }
  .ca-label { font-size: 10px; color: var(--text-4); text-transform: uppercase; letter-spacing: .3px; }
  .ca-value { font-size: 13px; color: var(--text); word-break: break-all; }

  .badge-accent { background: rgba(143, 227, 106, .15); color: var(--accent); }
  .badge-info { background: rgba(111, 175, 232, .15); color: var(--info); }
  .badge-muted { background: rgba(110, 122, 132, .15); color: var(--muted); }

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

  .path-cell { max-width: 260px; overflow: hidden; text-overflow: ellipsis; }
  .val-warn { color: var(--warn); }

  .empty-card {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    padding: 48px 24px; gap: 12px; text-align: center;
  }
  .empty-icon { color: var(--text-4); }
  .empty-title { font-size: 16px; font-weight: 600; color: var(--text-2); }
  .empty-hint { font-size: 13px; color: var(--text-4); max-width: 400px; line-height: 1.5; }
</style>
