<script>
  let { data } = $props();

  function fmtBytes(n) { if(!n) return '0 B'; if(n<1024) return n+' B'; if(n<1048576) return (n/1024).toFixed(1)+' KB'; return (n/1048576).toFixed(1)+' MB'; }
  function fmtTime(ms) { if(!ms) return '\u2014'; const d=new Date(ms); return d.toLocaleTimeString('en-GB',{hour12:false})+'.'+String(ms%1000).padStart(3,'0'); }

  let rulesFull = $derived(data?.rulesFull ?? []);
  let stats = $derived(data?.stats ?? []);

  let exporterRules = $derived(rulesFull.filter(r => r.exporter));

  let udsCount = $derived(exporterRules.filter(r => r.exporter.kind === 'uds').length);
  let tcpFramedCount = $derived(exporterRules.filter(r => r.exporter.kind === 'tcp').length);

  let totalExportDrops = $derived(
    stats.reduce((sum, s) => sum + (s.export_drops ?? 0), 0)
  );

  function findStats(ruleName) {
    return stats.find(s => s.rule_name === ruleName);
  }

  function exporterDest(exp) {
    if (exp.kind === 'uds') return exp.path ?? '\u2014';
    if (exp.kind === 'tcp') return exp.addr ?? '\u2014';
    return exp.path ?? exp.addr ?? '\u2014';
  }

  let showProtocolRef = $state(false);
</script>

<div class="exporters-page">
  <div class="page-header">
    <h1 class="page-title">Exporters</h1>
    <span class="page-subtitle mono">{exporterRules.length} configured</span>
  </div>

  <!-- Status Cards -->
  <div class="cards-grid">
    <div class="card stat-card">
      <div class="stat-label">Exporters</div>
      <div class="stat-value mono">{exporterRules.length}</div>
      <div class="stat-sub">configured</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">UDS</div>
      <div class="stat-value mono">{udsCount}</div>
      <div class="stat-sub">unix domain socket</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">TCP Framed</div>
      <div class="stat-value mono">{tcpFramedCount}</div>
      <div class="stat-sub">tcp stream export</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Export Drops</div>
      <div class="stat-value mono" class:val-warn={totalExportDrops > 0}>{totalExportDrops}</div>
      <div class="stat-sub">total across all rules</div>
    </div>
  </div>

  {#if exporterRules.length > 0}
    <!-- Exporters Table -->
    <div class="card table-section">
      <div class="section-title">Exporters</div>
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Rule</th>
              <th>Type</th>
              <th>Destination</th>
              <th>Export Drops</th>
              <th>Bytes C2S</th>
              <th>Bytes S2C</th>
            </tr>
          </thead>
          <tbody>
            {#each exporterRules as rule}
              {@const s = findStats(rule.name)}
              <tr>
                <td class="mono">{rule.name}</td>
                <td>
                  <span class="badge" class:badge-info={rule.exporter.kind === 'uds'} class:badge-accent={rule.exporter.kind === 'tcp'}>
                    {rule.exporter.kind === 'uds' ? 'UDS' : 'TCP'}
                  </span>
                </td>
                <td class="mono dest-cell">{exporterDest(rule.exporter)}</td>
                <td class="mono" class:val-warn={(s?.export_drops ?? 0) > 0}>{s?.export_drops ?? 0}</td>
                <td class="mono">{fmtBytes(s?.bytes_client_to_server)}</td>
                <td class="mono">{fmtBytes(s?.bytes_server_to_client)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>

    <!-- Protocol Reference Toggle -->
    <div class="card">
      <button class="section-toggle" onclick={() => showProtocolRef = !showProtocolRef}>
        <span class="section-title" style="margin-bottom: 0;">Export Protocol Reference</span>
        <span class="toggle-arrow" class:open={showProtocolRef}>{showProtocolRef ? '\u25B2' : '\u25BC'}</span>
      </button>

      {#if showProtocolRef}
        <div class="protocol-ref">
          <p class="ref-intro">
            Exported events use a length-prefixed binary framing protocol. Each frame is sent as a single message
            over the configured transport (UDS or TCP).
          </p>

          <div class="ref-table-wrap">
            <table class="ref-table">
              <thead>
                <tr>
                  <th>Offset</th>
                  <th>Size</th>
                  <th>Field</th>
                  <th>Description</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td class="mono">0</td>
                  <td class="mono">4</td>
                  <td class="mono">frame_len</td>
                  <td>Total frame length (big-endian u32), excluding this header</td>
                </tr>
                <tr>
                  <td class="mono">4</td>
                  <td class="mono">1</td>
                  <td class="mono">version</td>
                  <td>Protocol version byte (currently <span class="mono">0x01</span>)</td>
                </tr>
                <tr>
                  <td class="mono">5</td>
                  <td class="mono">1</td>
                  <td class="mono">event_kind</td>
                  <td>Event type: <span class="mono">0x01</span> = data C2S, <span class="mono">0x02</span> = data S2C, <span class="mono">0x03</span> = connect, <span class="mono">0x04</span> = disconnect</td>
                </tr>
                <tr>
                  <td class="mono">6</td>
                  <td class="mono">8</td>
                  <td class="mono">timestamp</td>
                  <td>Unix epoch milliseconds (big-endian u64)</td>
                </tr>
                <tr>
                  <td class="mono">14</td>
                  <td class="mono">2+N</td>
                  <td class="mono">rule_name</td>
                  <td>Length-prefixed string (big-endian u16 length + UTF-8 bytes)</td>
                </tr>
                <tr>
                  <td class="mono">14+2+N</td>
                  <td class="mono">...</td>
                  <td class="mono">payload</td>
                  <td>Remaining bytes are the event payload (raw proxied data or connection detail)</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      {/if}
    </div>
  {:else}
    <!-- Empty State -->
    <div class="card empty-card">
      <div class="empty-icon">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
          <polyline points="17 8 12 3 7 8"/>
          <line x1="12" y1="3" x2="12" y2="15"/>
        </svg>
      </div>
      <div class="empty-title">No exporters configured</div>
      <div class="empty-hint">Add an exporter to a rule to stream proxied traffic to an external consumer via UDS or TCP framed protocol.</div>
    </div>
  {/if}
</div>

<style>
  .exporters-page { display: flex; flex-direction: column; gap: 20px; }

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

  .badge-accent { background: rgba(143, 227, 106, .15); color: var(--accent); }
  .badge-info { background: rgba(111, 175, 232, .15); color: var(--info); }

  .val-warn { color: var(--warn); }

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

  .dest-cell { max-width: 340px; overflow: hidden; text-overflow: ellipsis; }

  .section-toggle {
    display: flex; align-items: center; justify-content: space-between; width: 100%;
    background: none; border: none; cursor: pointer; padding: 0; color: var(--text);
  }
  .toggle-arrow { font-size: 11px; color: var(--text-4); transition: transform .15s; }

  .protocol-ref { margin-top: 16px; padding-top: 16px; border-top: 1px solid var(--bg-4); }
  .ref-intro { font-size: 13px; color: var(--text-3); line-height: 1.5; margin-bottom: 16px; }
  .ref-table-wrap { overflow-x: auto; }
  .ref-table tbody td { white-space: normal; }

  .empty-card {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    padding: 48px 24px; gap: 12px; text-align: center;
  }
  .empty-icon { color: var(--text-4); }
  .empty-title { font-size: 16px; font-weight: 600; color: var(--text-2); }
  .empty-hint { font-size: 13px; color: var(--text-4); max-width: 400px; line-height: 1.5; }
</style>
