<script>
  import { fetchNftStatus, fetchNftPreview, applyNft, rollbackNft } from '../lib/api.js';

  let { data, onRefresh } = $props();

  function fmtBytes(n) { if(!n) return '0 B'; if(n<1024) return n+' B'; if(n<1048576) return (n/1024).toFixed(1)+' KB'; return (n/1048576).toFixed(1)+' MB'; }
  function fmtTime(ms) { if(!ms) return '\u2014'; const d=new Date(ms); return d.toLocaleTimeString('en-GB',{hour12:false})+'.'+String(ms%1000).padStart(3,'0'); }

  let rules = $derived(data?.rules ?? []);

  let nftAvailable = $state(null);
  let nftStatus = $state(null);
  let preview = $state('');
  let applying = $state(false);
  let rollingBack = $state(false);
  let loadingPreview = $state(false);
  let copySuccess = $state(false);
  let error = $state('');

  $effect(() => {
    checkNftStatus();
  });

  async function checkNftStatus() {
    try {
      const res = await fetchNftStatus();
      nftStatus = res;
      nftAvailable = res.available ?? true;
    } catch {
      nftAvailable = false;
    }
  }

  async function loadPreview() {
    loadingPreview = true;
    error = '';
    try {
      const res = await fetchNftPreview();
      preview = res.ruleset ?? res.preview ?? '';
    } catch (e) {
      error = 'Failed to load preview: ' + e.message;
    }
    loadingPreview = false;
  }

  async function handleApply() {
    if (!confirm('Apply nftables rules? This will modify the "ip harpoon" table in the kernel packet filter. Existing rules in that table will be replaced.')) return;
    applying = true;
    error = '';
    try {
      await applyNft();
      await checkNftStatus();
      onRefresh?.();
    } catch (e) {
      error = 'Apply failed: ' + e.message;
    }
    applying = false;
  }

  async function handleRollback() {
    if (!confirm('Rollback and delete the "ip harpoon" nftables table? This will remove all harpoon-managed packet filtering rules.')) return;
    rollingBack = true;
    error = '';
    try {
      await rollbackNft();
      preview = '';
      await checkNftStatus();
      onRefresh?.();
    } catch (e) {
      error = 'Rollback failed: ' + e.message;
    }
    rollingBack = false;
  }

  async function copyPreview() {
    try {
      await navigator.clipboard.writeText(preview);
      copySuccess = true;
      setTimeout(() => copySuccess = false, 2000);
    } catch { /* clipboard not available */ }
  }
</script>

<div class="nft-page">
  <div class="page-header">
    <h1 class="page-title">nftables</h1>
    <span class="page-subtitle mono">kernel packet filtering</span>
  </div>

  <!-- Status Cards -->
  <div class="cards-grid">
    <div class="card stat-card">
      <div class="stat-label">nft Status</div>
      <div class="stat-value">
        {#if nftAvailable === null}
          <span class="mono" style="color: var(--text-4);">checking...</span>
        {:else if nftAvailable}
          <span class="status-dot available"></span>
          <span>Available</span>
        {:else}
          <span class="status-dot"></span>
          <span>Unavailable</span>
        {/if}
      </div>
      <div class="stat-sub">{nftStatus?.detail ?? 'nftables subsystem'}</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Managed Table</div>
      <div class="stat-value mono">ip harpoon</div>
      <div class="stat-sub">isolated namespace</div>
    </div>
    <div class="card stat-card">
      <div class="stat-label">Proxy Rules</div>
      <div class="stat-value mono">{rules.length}</div>
      <div class="stat-sub">active rules to translate</div>
    </div>
  </div>

  <!-- Warning Panel -->
  <div class="warning-panel">
    <div class="warning-icon">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
        <line x1="12" y1="9" x2="12" y2="13"/>
        <line x1="12" y1="17" x2="12.01" y2="17"/>
      </svg>
    </div>
    <div class="warning-text">
      <strong>nftables operations modify kernel packet filtering.</strong>
      Changes only affect the <span class="mono">ip harpoon</span> table and will not touch other nft tables or iptables rules.
      Rollback deletes the entire harpoon table from the kernel.
    </div>
  </div>

  {#if error}
    <div class="error-banner">{error}</div>
  {/if}

  <!-- Ruleset Preview -->
  <div class="card">
    <div class="preview-header">
      <span class="section-title" style="margin-bottom: 0;">Generated Ruleset Preview</span>
      <div class="preview-actions">
        <button class="btn btn-sm" onclick={loadPreview} disabled={loadingPreview}>
          {loadingPreview ? 'Loading...' : 'Refresh'}
        </button>
        {#if preview}
          <button class="btn btn-sm" onclick={copyPreview}>
            {copySuccess ? 'Copied!' : 'Copy'}
          </button>
        {/if}
      </div>
    </div>

    {#if preview}
      <pre class="code-block mono">{preview}</pre>
    {:else}
      <div class="preview-empty">
        Click <strong>Refresh</strong> to generate and preview the nftables ruleset for current proxy rules.
      </div>
    {/if}
  </div>

  <!-- Actions -->
  <div class="card">
    <div class="section-title">Actions</div>
    <div class="actions-row">
      <button class="btn btn-accent" onclick={handleApply} disabled={applying || !nftAvailable}>
        {applying ? 'Applying...' : 'Apply Rules'}
      </button>
      <button class="btn btn-danger" onclick={handleRollback} disabled={rollingBack || !nftAvailable}>
        {rollingBack ? 'Rolling back...' : 'Rollback / Delete Table'}
      </button>
    </div>
  </div>

  <!-- Supported Actions Reference -->
  <div class="card">
    <div class="section-title">Supported Actions</div>
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Action</th>
            <th>Description</th>
            <th>Example nft Rule</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td><span class="badge badge-tcp">REDIRECT</span></td>
            <td>Redirect incoming traffic on a port to the local proxy listener</td>
            <td class="mono example-cell">tcp dport 80 redirect to :8080</td>
          </tr>
          <tr>
            <td><span class="badge badge-udp">DNAT</span></td>
            <td>Destination NAT to rewrite the target address for forwarded traffic</td>
            <td class="mono example-cell">ip daddr 10.0.0.1 tcp dport 443 dnat to 127.0.0.1:8443</td>
          </tr>
          <tr>
            <td><span class="badge badge-warn">TPROXY</span></td>
            <td>Transparent proxy via TPROXY target, preserving original destination</td>
            <td class="mono example-cell">tcp dport 80 tproxy to :8080 meta mark set 1</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</div>

<style>
  .nft-page { display: flex; flex-direction: column; gap: 20px; }

  .page-header { display: flex; align-items: baseline; gap: 12px; }
  .page-title { font-size: 22px; font-weight: 700; color: var(--text); }
  .page-subtitle { font-size: 12px; color: var(--text-3); }

  .cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
    gap: 12px;
  }

  .stat-card { display: flex; flex-direction: column; gap: 4px; }
  .stat-label { font-size: 11px; color: var(--text-3); text-transform: uppercase; letter-spacing: .5px; }
  .stat-value { font-size: 18px; font-weight: 700; color: var(--text); display: flex; align-items: center; gap: 8px; }
  .stat-sub { font-size: 11px; color: var(--text-4); }

  .status-dot {
    width: 8px; height: 8px; border-radius: 50%; background: var(--err); flex-shrink: 0;
  }
  .status-dot.available {
    background: var(--ok); box-shadow: 0 0 6px rgba(112, 217, 138, 0.5);
  }

  .warning-panel {
    display: flex; gap: 12px; padding: 14px 16px;
    background: rgba(230, 193, 90, .06);
    border: 1px solid rgba(230, 193, 90, .25);
    border-radius: var(--radius);
  }
  .warning-icon { color: var(--warn); flex-shrink: 0; padding-top: 1px; }
  .warning-text { font-size: 13px; color: var(--text-2); line-height: 1.55; }
  .warning-text strong { color: var(--warn); }

  .error-banner {
    padding: 10px 14px; background: rgba(226, 109, 109, .1);
    border: 1px solid rgba(226, 109, 109, .3); border-radius: var(--radius);
    font-size: 13px; color: var(--err);
  }

  .preview-header {
    display: flex; align-items: center; justify-content: space-between;
    margin-bottom: 12px;
  }
  .preview-actions { display: flex; gap: 6px; }

  .code-block {
    background: var(--bg-1); border: 1px solid var(--bg-4); border-radius: var(--radius-sm);
    padding: 14px 16px; font-size: 12px; line-height: 1.6; color: var(--text-2);
    overflow-x: auto; white-space: pre; max-height: 400px; overflow-y: auto;
  }

  .preview-empty {
    padding: 24px 0; text-align: center; color: var(--text-4); font-size: 13px;
  }

  .actions-row { display: flex; gap: 10px; flex-wrap: wrap; }

  .table-wrap { overflow-x: auto; }

  table { width: 100%; border-collapse: collapse; font-size: 12px; }
  thead th {
    text-align: left; padding: 8px 10px; font-size: 10px; font-weight: 600;
    color: var(--text-3); text-transform: uppercase; letter-spacing: .5px;
    border-bottom: 1px solid var(--bg-4); white-space: nowrap;
  }
  tbody td {
    padding: 7px 10px; border-bottom: 1px solid var(--bg-3); color: var(--text-2);
  }
  tbody tr:hover { background: var(--bg-3); }
  tbody tr:last-child td { border-bottom: none; }

  .example-cell { white-space: normal; word-break: break-all; max-width: 420px; font-size: 11px; }
</style>
