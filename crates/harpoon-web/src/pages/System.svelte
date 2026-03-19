<script>
  let { data } = $props();

  function fmtBytes(n) { if(!n) return '0 B'; if(n<1024) return n+' B'; if(n<1048576) return (n/1024).toFixed(1)+' KB'; if(n<1073741824) return (n/1048576).toFixed(1)+' MB'; return (n/1073741824).toFixed(2)+' GB'; }
  function fmtUptime(s) { if(!s) return '\u2014'; if(s<60) return s+'s'; if(s<3600) return Math.floor(s/60)+'m '+(s%60)+'s'; return Math.floor(s/3600)+'h '+Math.floor((s%3600)/60)+'m'; }
  function fmtTime(ms) { if(!ms) return '\u2014'; const d=new Date(ms); return d.toLocaleTimeString('en-GB',{hour12:false})+'.'+String(ms%1000).padStart(3,'0'); }

  let status = $derived(data?.status ?? {});
  let stats = $derived(data?.stats ?? []);
  let rules = $derived(data?.rules ?? []);

  let isRunning = $derived(status.running ?? false);
  let uptimeStr = $derived(fmtUptime(status.uptime_secs));
  let totalBytesIn = $derived(stats.reduce((s, r) => s + (r.bytes_client_to_server ?? 0), 0));
  let totalBytesOut = $derived(stats.reduce((s, r) => s + (r.bytes_server_to_client ?? 0), 0));
  let totalTraffic = $derived(totalBytesIn + totalBytesOut);

  let featureList = $derived(() => {
    const features = [];
    const rulesFull = data?.rulesFull ?? [];
    const hasTcp = rulesFull.some(r => r.protocol === 'tcp');
    const hasUdp = rulesFull.some(r => r.protocol === 'udp');
    const hasTls = rulesFull.some(r => r.tls);
    const hasFilters = rulesFull.some(r => r.filters && r.filters.length > 0);
    const hasDuplicate = rulesFull.some(r => r.duplicate);
    const hasExporter = rulesFull.some(r => r.exporter);

    if (hasTcp) features.push({ label: 'TCP Proxy', badge: 'badge-tcp' });
    if (hasUdp) features.push({ label: 'UDP Proxy', badge: 'badge-udp' });
    if (hasTls) features.push({ label: 'TLS', badge: 'badge-warn' });
    if (hasFilters) features.push({ label: 'Filters', badge: 'badge-ok' });
    if (hasDuplicate) features.push({ label: 'Duplication', badge: 'badge-warn' });
    if (hasExporter) features.push({ label: 'Export', badge: 'badge-ok' });
    return features;
  });
</script>

<div class="system-page">
  <div class="page-header">
    <h1 class="page-title">System</h1>
    <span class="page-subtitle mono">harpoon operator console</span>
  </div>

  <!-- Status Cards Grid -->
  <div class="cards-grid">
    <div class="card stat-card">
      <div class="stat-label">Version</div>
      <div class="stat-value mono">0.1.0</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Uptime</div>
      <div class="stat-value mono">{uptimeStr}</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Status</div>
      <div class="stat-value">
        <span class="status-dot" class:running={isRunning}></span>
        {isRunning ? 'Running' : 'Stopped'}
      </div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Total Traffic</div>
      <div class="stat-value mono">{fmtBytes(totalTraffic)}</div>
      <div class="stat-sub">{fmtBytes(totalBytesIn)} in / {fmtBytes(totalBytesOut)} out</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Rules</div>
      <div class="stat-value mono">{status.rules_count ?? rules.length}</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Config Path</div>
      <div class="stat-value config-path mono">{status.config_path ?? 'unknown'}</div>
    </div>
  </div>

  <!-- Features -->
  <div class="card">
    <div class="section-title">Active Features</div>
    {#if featureList().length === 0}
      <div class="empty-state">No features active. Configure rules to enable features.</div>
    {:else}
      <div class="features-grid">
        {#each featureList() as feat}
          <span class="badge {feat.badge}">{feat.label}</span>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Build Info -->
  <div class="card">
    <div class="section-title">Build Information</div>
    <div class="info-grid">
      <div class="info-item">
        <span class="info-label">Binary</span>
        <span class="info-value mono">harpoon</span>
      </div>
      <div class="info-item">
        <span class="info-label">Version</span>
        <span class="info-value mono">0.1.0</span>
      </div>
      <div class="info-item">
        <span class="info-label">Platform</span>
        <span class="info-value mono">linux/x86_64</span>
      </div>
      <div class="info-item">
        <span class="info-label">Runtime</span>
        <span class="info-value mono">tokio (async)</span>
      </div>
      <div class="info-item">
        <span class="info-label">Language</span>
        <span class="info-value mono">Rust</span>
      </div>
      <div class="info-item">
        <span class="info-label">Web UI</span>
        <span class="info-value mono">Svelte 5</span>
      </div>
    </div>
  </div>

  <!-- Control Plane -->
  <div class="card">
    <div class="section-title">Control Plane</div>
    <div class="info-grid">
      <div class="info-item">
        <span class="info-label">API Endpoint</span>
        <span class="info-value mono">/api/*</span>
      </div>
      <div class="info-item">
        <span class="info-label">Authentication</span>
        <span class="info-value mono">Bearer Token (JWT)</span>
      </div>
      <div class="info-item">
        <span class="info-label">Auto-Refresh</span>
        <span class="info-value mono">3s interval</span>
      </div>
      <div class="info-item">
        <span class="info-label">Config Format</span>
        <span class="info-value mono">TOML</span>
      </div>
      <div class="info-item">
        <span class="info-label">Hot Reload</span>
        <span class="info-value">
          <span class="badge badge-ok">supported</span>
        </span>
      </div>
      <div class="info-item">
        <span class="info-label">NFTables</span>
        <span class="info-value">
          <span class="badge badge-ok">integrated</span>
        </span>
      </div>
    </div>
  </div>
</div>

<style>
  .system-page { display: flex; flex-direction: column; gap: 20px; }

  .page-header { display: flex; align-items: baseline; gap: 12px; }
  .page-title { font-size: 22px; font-weight: 700; color: var(--text); }
  .page-subtitle { font-size: 12px; color: var(--text-3); }

  .cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 12px;
  }

  .stat-card { display: flex; flex-direction: column; gap: 4px; }
  .stat-label { font-size: 11px; color: var(--text-3); text-transform: uppercase; letter-spacing: .5px; }
  .stat-value { font-size: 18px; font-weight: 700; color: var(--text); display: flex; align-items: center; gap: 8px; }
  .stat-sub { font-size: 11px; color: var(--text-4); }

  .config-path { font-size: 13px; font-weight: 500; word-break: break-all; }

  .status-dot {
    width: 8px; height: 8px; border-radius: 50%; background: var(--err); flex-shrink: 0;
  }
  .status-dot.running {
    background: var(--ok); box-shadow: 0 0 6px rgba(112, 217, 138, 0.5);
  }

  .features-grid { display: flex; flex-wrap: wrap; gap: 8px; }

  .info-grid {
    display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 12px;
  }
  .info-item { display: flex; flex-direction: column; gap: 3px; }
  .info-label { font-size: 10px; color: var(--text-4); text-transform: uppercase; letter-spacing: .3px; }
  .info-value { font-size: 13px; color: var(--text); }

  .empty-state { color: var(--text-4); font-size: 13px; padding: 12px 0; }
</style>
