<script>
  import { reload, fetchConfigToml } from '../lib/api.js';

  let { data, onRefresh } = $props();

  function fmtBytes(n) { if(!n) return '0 B'; if(n<1024) return n+' B'; if(n<1048576) return (n/1024).toFixed(1)+' KB'; if(n<1073741824) return (n/1048576).toFixed(1)+' MB'; return (n/1073741824).toFixed(2)+' GB'; }
  function fmtUptime(s) { if(!s) return '\u2014'; if(s<60) return s+'s'; if(s<3600) return Math.floor(s/60)+'m '+(s%60)+'s'; return Math.floor(s/3600)+'h '+Math.floor((s%3600)/60)+'m'; }
  function fmtTime(ms) { if(!ms) return '\u2014'; const d=new Date(ms); return d.toLocaleTimeString('en-GB',{hour12:false})+'.'+String(ms%1000).padStart(3,'0'); }

  let status = $derived(data?.status ?? {});
  let rulesFull = $derived(data?.rulesFull ?? []);
  let rules = $derived(data?.rules ?? []);

  let reloading = $state(false);
  let showRawToml = $state(false);
  let rawToml = $state('');
  let loadingToml = $state(false);
  let tomlError = $state('');

  async function handleReload() {
    reloading = true;
    try {
      await reload();
      onRefresh?.();
    } catch (e) {
      alert('Reload failed: ' + e.message);
    }
    reloading = false;
  }

  async function handleViewToml() {
    if (showRawToml && rawToml) {
      showRawToml = false;
      return;
    }
    loadingToml = true;
    tomlError = '';
    try {
      const result = await fetchConfigToml();
      rawToml = result.toml ?? result.content ?? JSON.stringify(result, null, 2);
      showRawToml = true;
    } catch (e) {
      tomlError = 'Failed to fetch TOML: ' + e.message;
    }
    loadingToml = false;
  }

  function handleDownloadToml() {
    if (!rawToml) {
      // Fetch first then download
      fetchConfigToml().then(result => {
        const content = result.toml ?? result.content ?? JSON.stringify(result, null, 2);
        downloadFile(content, 'harpoon.toml', 'text/plain');
      }).catch(e => {
        alert('Failed to fetch config: ' + e.message);
      });
    } else {
      downloadFile(rawToml, 'harpoon.toml', 'text/plain');
    }
  }

  function downloadFile(content, filename, mime) {
    const blob = new Blob([content], { type: mime });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
  }
</script>

<div class="config-page">
  <div class="page-header">
    <h1 class="page-title">Configuration</h1>
  </div>

  <!-- Config Info -->
  <div class="info-row">
    <div class="card info-card">
      <div class="info-label">Config Path</div>
      <div class="info-value mono">{status.config_path ?? 'unknown'}</div>
    </div>
    <div class="card info-card">
      <div class="info-label">Rule Count</div>
      <div class="info-value mono">{status.rules_count ?? rules.length}</div>
    </div>
  </div>

  <!-- Actions -->
  <div class="actions-row">
    <button class="btn btn-accent" onclick={handleReload} disabled={reloading}>
      {reloading ? 'Reloading...' : 'Reload Config'}
    </button>
    <button class="btn" onclick={handleDownloadToml}>Download TOML</button>
    <button class="btn" onclick={handleViewToml} disabled={loadingToml}>
      {loadingToml ? 'Loading...' : showRawToml ? 'Hide Raw TOML' : 'View Raw TOML'}
    </button>
  </div>

  {#if tomlError}
    <div class="error-banner">{tomlError}</div>
  {/if}

  <!-- Effective Rules Table -->
  <div class="card table-section">
    <div class="section-title">Effective Rules</div>
    {#if rulesFull.length === 0}
      <div class="empty-state">No rules in current configuration.</div>
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
              <th>Duplicate</th>
              <th>Exporter</th>
              <th>TLS</th>
              <th>UDP Mode</th>
              <th>Idle Timeout</th>
            </tr>
          </thead>
          <tbody>
            {#each rulesFull as rule}
              <tr>
                <td class="mono">{rule.name}</td>
                <td>
                  <span class="badge" class:badge-tcp={rule.protocol === 'tcp'} class:badge-udp={rule.protocol === 'udp'}>
                    {rule.protocol}
                  </span>
                </td>
                <td class="mono">{rule.listen}</td>
                <td class="mono">{rule.target}</td>
                <td>{rule.filters?.length ?? 0}</td>
                <td class="mono">{rule.duplicate ?? '\u2014'}</td>
                <td>
                  {#if rule.exporter}
                    <span class="badge badge-ok">{rule.exporter.kind}</span>
                  {:else}
                    <span class="muted-text">\u2014</span>
                  {/if}
                </td>
                <td>
                  {#if rule.tls}
                    <span class="badge badge-warn">{rule.tls.mode ?? 'on'}</span>
                  {:else}
                    <span class="muted-text">\u2014</span>
                  {/if}
                </td>
                <td class="mono">{rule.protocol === 'udp' ? (rule.udp_source_mode ?? 'connected') : '\u2014'}</td>
                <td class="mono">{rule.protocol === 'udp' ? (rule.idle_timeout_secs ?? 30) + 's' : '\u2014'}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>

  <!-- Raw TOML Section -->
  {#if showRawToml && rawToml}
    <div class="card toml-section">
      <div class="section-title">Raw Configuration (TOML)</div>
      <div class="toml-wrap">
        <pre class="toml-block"><code class="mono">{rawToml}</code></pre>
      </div>
    </div>
  {/if}
</div>

<style>
  .config-page { display: flex; flex-direction: column; gap: 20px; }

  .page-header { display: flex; align-items: baseline; gap: 12px; }
  .page-title { font-size: 22px; font-weight: 700; color: var(--text); }

  .info-row { display: flex; gap: 12px; flex-wrap: wrap; }
  .info-card { min-width: 200px; }
  .info-label { font-size: 11px; color: var(--text-3); text-transform: uppercase; letter-spacing: .5px; margin-bottom: 4px; }
  .info-value { font-size: 15px; font-weight: 600; color: var(--text); }

  .actions-row { display: flex; gap: 8px; flex-wrap: wrap; }

  .error-banner {
    padding: 10px 14px; background: rgba(226,109,109,.1); border: 1px solid rgba(226,109,109,.3);
    border-radius: var(--radius-sm); color: var(--err); font-size: 13px;
  }

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

  .muted-text { color: var(--text-4); }

  .toml-section { overflow: hidden; }
  .toml-wrap { overflow-x: auto; max-height: 500px; overflow-y: auto; }
  .toml-block {
    margin: 0; padding: 16px; background: var(--bg-1); border-radius: var(--radius-sm);
    font-size: 12px; line-height: 1.6; color: var(--text-2); white-space: pre;
    border: 1px solid var(--bg-4);
  }

  .empty-state { color: var(--text-4); font-size: 13px; padding: 20px 0; text-align: center; }
</style>
