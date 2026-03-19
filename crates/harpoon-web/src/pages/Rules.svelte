<script>
  import { deleteRule } from '../lib/api.js';

  let { data, onRefresh, onEditPipeline, onEditRule, onCreateRule } = $props();

  function fmtBytes(n) { if(!n) return '0 B'; if(n<1024) return n+' B'; if(n<1048576) return (n/1024).toFixed(1)+' KB'; if(n<1073741824) return (n/1048576).toFixed(1)+' MB'; return (n/1073741824).toFixed(2)+' GB'; }
  function fmtUptime(s) { if(!s) return '\u2014'; if(s<60) return s+'s'; if(s<3600) return Math.floor(s/60)+'m '+(s%60)+'s'; return Math.floor(s/3600)+'h '+Math.floor((s%3600)/60)+'m'; }
  function fmtTime(ms) { if(!ms) return '\u2014'; const d=new Date(ms); return d.toLocaleTimeString('en-GB',{hour12:false})+'.'+String(ms%1000).padStart(3,'0'); }

  let rules = $derived(data?.rules ?? []);
  let rulesFull = $derived(data?.rulesFull ?? []);
  let stats = $derived(data?.stats ?? []);

  let selectedName = $state(null);
  let deleting = $state(null);

  let selectedRule = $derived(
    selectedName ? rulesFull.find(r => r.name === selectedName) ?? rules.find(r => r.name === selectedName) : null
  );

  let selectedStats = $derived(
    selectedName ? stats.find(s => s.rule_name === selectedName) : null
  );

  function selectRule(name) {
    selectedName = selectedName === name ? null : name;
  }

  function handleEdit(rule) {
    // Build a pipeline preset from the rule for the editor
    const preset = {
      name: rule.name,
      nodes: [
        { id: 1, kind: 'source', config: { protocol: rule.protocol, listen: rule.listen, idle_timeout_secs: rule.idle_timeout_secs || 30, udp_source_mode: rule.udp_source_mode || 'connected' } },
        { id: 2, kind: 'forward', config: { target: rule.target } },
      ],
      edges: [{ from: 1, to: 2 }],
    };
    let nextId = 3;
    if (rule.filters && rule.filters.length > 0) {
      rule.filters.forEach(f => {
        preset.nodes.push({ id: nextId, kind: 'filter', config: { ...f } });
        preset.edges.push({ from: 1, to: nextId });
        nextId++;
      });
    }
    if (rule.duplicate) {
      preset.nodes.push({ id: nextId, kind: 'duplicate', config: { target: rule.duplicate } });
      preset.edges.push({ from: 1, to: nextId });
      nextId++;
    }
    if (rule.exporter) {
      preset.nodes.push({ id: nextId, kind: 'export', config: { ...rule.exporter } });
      preset.edges.push({ from: 2, to: nextId });
      nextId++;
    }
    onEditPipeline?.(preset);
  }

  async function handleDelete(name) {
    if (!confirm(`Delete rule "${name}"? This will stop all active connections for this rule.`)) return;
    deleting = name;
    try {
      await deleteRule(name);
      if (selectedName === name) selectedName = null;
      onRefresh?.();
    } catch (e) {
      alert('Delete failed: ' + e.message);
    }
    deleting = null;
  }
</script>

<div class="rules-page">
  <div class="page-header">
    <h1 class="page-title">Rules</h1>
    <span class="page-count mono">{rules.length} configured</span>
    <div style="flex:1;"></div>
    <button class="btn btn-accent" onclick={() => onCreateRule?.()}>+ Create Rule</button>
  </div>

  <div class="rules-layout">
    <!-- Rules List -->
    <div class="rules-list">
      <div class="card table-section">
        {#if rules.length === 0}
          <div class="empty-state">No rules configured.</div>
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
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {#each rules as rule}
                  <tr class:selected={selectedName === rule.name}
                      onclick={() => selectRule(rule.name)}>
                    <td class="mono">{rule.name}</td>
                    <td>
                      <span class="badge" class:badge-tcp={rule.protocol === 'tcp'} class:badge-udp={rule.protocol === 'udp'}>
                        {rule.protocol}
                      </span>
                    </td>
                    <td class="mono">{rule.listen}</td>
                    <td class="mono">{rule.target}</td>
                    <td>{rule.filters_count ?? 0}</td>
                    <td class="actions-cell" onclick={(e) => e.stopPropagation()}>
                      <button class="btn btn-sm" onclick={() => onEditRule?.(rulesFull.find(r => r.name === rule.name) ?? rule)}>Edit</button>
                      <button class="btn btn-sm" onclick={() => handleEdit(rulesFull.find(r => r.name === rule.name) ?? rule)}>Pipeline</button>
                      <button class="btn btn-sm btn-danger" onclick={() => handleDelete(rule.name)} disabled={deleting === rule.name}>
                        {deleting === rule.name ? '...' : 'Del'}
                      </button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>
    </div>

    <!-- Detail Panel -->
    {#if selectedRule}
      <div class="detail-panel">
        <div class="card">
          <div class="detail-header">
            <h2 class="detail-name mono">{selectedRule.name}</h2>
            <span class="badge" class:badge-tcp={selectedRule.protocol === 'tcp'} class:badge-udp={selectedRule.protocol === 'udp'}>
              {selectedRule.protocol}
            </span>
          </div>

          <!-- Config Section -->
          <div class="detail-section">
            <div class="detail-section-title">Configuration</div>
            <div class="detail-grid">
              <div class="detail-item">
                <span class="detail-label">Listen</span>
                <span class="detail-value mono">{selectedRule.listen}</span>
              </div>
              <div class="detail-item">
                <span class="detail-label">Target</span>
                <span class="detail-value mono">{selectedRule.target}</span>
              </div>
              {#if selectedRule.protocol === 'udp'}
                <div class="detail-item">
                  <span class="detail-label">Source Mode</span>
                  <span class="detail-value mono">{selectedRule.udp_source_mode ?? 'connected'}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">Idle Timeout</span>
                  <span class="detail-value mono">{selectedRule.idle_timeout_secs ?? 30}s</span>
                </div>
              {/if}
              {#if selectedRule.duplicate}
                <div class="detail-item">
                  <span class="detail-label">Duplicate To</span>
                  <span class="detail-value mono">{selectedRule.duplicate}</span>
                </div>
              {/if}
            </div>
          </div>

          <!-- TLS Section -->
          {#if selectedRule.tls}
            <div class="detail-section">
              <div class="detail-section-title">TLS</div>
              <div class="detail-grid">
                <div class="detail-item">
                  <span class="detail-label">Mode</span>
                  <span class="detail-value mono">{selectedRule.tls.mode ?? 'terminate'}</span>
                </div>
              </div>
            </div>
          {/if}

          <!-- Exporter Section -->
          {#if selectedRule.exporter}
            <div class="detail-section">
              <div class="detail-section-title">Exporter</div>
              <div class="detail-grid">
                <div class="detail-item">
                  <span class="detail-label">Kind</span>
                  <span class="detail-value mono">{selectedRule.exporter.kind}</span>
                </div>
                {#if selectedRule.exporter.path}
                  <div class="detail-item">
                    <span class="detail-label">Path</span>
                    <span class="detail-value mono">{selectedRule.exporter.path}</span>
                  </div>
                {/if}
                {#if selectedRule.exporter.addr}
                  <div class="detail-item">
                    <span class="detail-label">Address</span>
                    <span class="detail-value mono">{selectedRule.exporter.addr}</span>
                  </div>
                {/if}
              </div>
            </div>
          {/if}

          <!-- Filters Section -->
          {#if selectedRule.filters && selectedRule.filters.length > 0}
            <div class="detail-section">
              <div class="detail-section-title">Filters ({selectedRule.filters.length})</div>
              <div class="filters-list">
                {#each selectedRule.filters as filter, i}
                  <div class="filter-item">
                    <span class="filter-idx mono">{i + 1}</span>
                    <span class="badge badge-warn">{filter.kind ?? 'substr'}</span>
                    <span class="mono filter-pattern">{filter.pattern}</span>
                    <span class="filter-meta">{filter.direction ?? 'both'} / {filter.action ?? 'drop'}</span>
                  </div>
                {/each}
              </div>
            </div>
          {/if}

          <!-- Stats Section -->
          {#if selectedStats}
            <div class="detail-section">
              <div class="detail-section-title">Live Stats</div>
              <div class="detail-grid">
                <div class="detail-item">
                  <span class="detail-label">Bytes In</span>
                  <span class="detail-value mono">{fmtBytes(selectedStats.bytes_client_to_server)}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">Bytes Out</span>
                  <span class="detail-value mono">{fmtBytes(selectedStats.bytes_server_to_client)}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">TCP Conn</span>
                  <span class="detail-value mono">{selectedStats.active_tcp_connections ?? 0}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">UDP Sess</span>
                  <span class="detail-value mono">{selectedStats.active_udp_sessions ?? 0}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">Dropped</span>
                  <span class="detail-value mono">{selectedStats.dropped_packets ?? 0}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">Filter Hits</span>
                  <span class="detail-value mono">{selectedStats.filter_matches ?? 0}</span>
                </div>
                <div class="detail-item">
                  <span class="detail-label">Export Drops</span>
                  <span class="detail-value mono">{selectedStats.export_drops ?? 0}</span>
                </div>
              </div>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .rules-page { display: flex; flex-direction: column; gap: 16px; }

  .page-header { display: flex; align-items: baseline; gap: 12px; }
  .page-title { font-size: 22px; font-weight: 700; color: var(--text); }
  .page-count { font-size: 12px; color: var(--text-3); }

  .rules-layout { display: grid; grid-template-columns: 1fr 340px; gap: 16px; }
  .rules-layout:not(:has(.detail-panel)) { grid-template-columns: 1fr; }

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
    white-space: nowrap; cursor: pointer;
  }
  tbody tr:hover { background: var(--bg-3); }
  tbody tr:last-child td { border-bottom: none; }
  tbody tr.selected { background: rgba(143, 227, 106, 0.06); }
  tbody tr.selected td { border-bottom-color: rgba(143, 227, 106, 0.1); }

  .actions-cell { display: flex; gap: 6px; cursor: default; }

  .detail-panel { min-width: 0; }
  .detail-header { display: flex; align-items: center; gap: 10px; margin-bottom: 16px; }
  .detail-name { font-size: 16px; font-weight: 700; color: var(--accent); }

  .detail-section { margin-top: 16px; padding-top: 16px; border-top: 1px solid var(--bg-4); }
  .detail-section-title { font-size: 11px; font-weight: 600; color: var(--text-3); text-transform: uppercase; letter-spacing: .5px; margin-bottom: 10px; }

  .detail-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .detail-item { display: flex; flex-direction: column; gap: 2px; }
  .detail-label { font-size: 10px; color: var(--text-4); text-transform: uppercase; letter-spacing: .3px; }
  .detail-value { font-size: 13px; color: var(--text); }

  .filters-list { display: flex; flex-direction: column; gap: 6px; }
  .filter-item {
    display: flex; align-items: center; gap: 8px; padding: 6px 8px;
    background: var(--bg-3); border-radius: var(--radius-sm); font-size: 12px;
  }
  .filter-idx { font-size: 10px; color: var(--text-4); width: 16px; }
  .filter-pattern { color: var(--text); flex: 1; overflow: hidden; text-overflow: ellipsis; }
  .filter-meta { font-size: 10px; color: var(--text-4); white-space: nowrap; }

  .empty-state { color: var(--text-4); font-size: 13px; padding: 20px 0; text-align: center; }
</style>
