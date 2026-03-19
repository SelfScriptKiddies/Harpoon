<script>
  import { createRule, updateRule } from '../lib/api.js';
  import PipelineEditor from './PipelineEditor.svelte';

  let { rule = null, onSave, onCancel } = $props();

  let mode = $state('simple'); // simple | pipeline | summary
  let isEditing = $derived(!!rule?.name);
  let error = $state('');
  let saving = $state(false);

  // Form state — initialized from rule or defaults
  let name = $state(rule?.name || '');
  let protocol = $state(rule?.protocol || 'tcp');
  let listen = $state(rule?.listen || '');
  let target = $state(rule?.target || '');
  let duplicate = $state(rule?.duplicate || '');
  let idleTimeout = $state(rule?.idle_timeout_secs || 30);
  let udpSourceMode = $state(rule?.udp_source_mode || 'proxy');

  // TLS
  let tlsEnabled = $state(!!rule?.tls);
  let tlsMode = $state(rule?.tls?.mode || 'terminate');
  let tlsCaCert = $state(rule?.tls?.ca_cert || '');
  let tlsCaKey = $state(rule?.tls?.ca_key || '');

  // Exporter
  let exporterEnabled = $state(!!rule?.exporter);
  let exporterKind = $state(rule?.exporter?.kind || 'tcp');
  let exporterPath = $state(rule?.exporter?.path || '');
  let exporterAddr = $state(rule?.exporter?.addr || '');

  // Filters
  let filters = $state(rule?.filters?.map(f => ({ ...f })) || []);

  function addFilter() {
    filters = [...filters, { kind: 'substr', pattern: '', direction: 'both', action: 'drop' }];
  }
  function removeFilter(i) {
    filters = filters.filter((_, idx) => idx !== i);
  }
  function moveFilter(i, dir) {
    if (i + dir < 0 || i + dir >= filters.length) return;
    const arr = [...filters];
    [arr[i], arr[i + dir]] = [arr[i + dir], arr[i]];
    filters = arr;
  }

  // Build rule payload
  function buildRule() {
    const r = { name, protocol, listen, target };
    r.filters = filters.filter(f => f.pattern.trim());
    r.duplicate = duplicate.trim() || null;
    r.idle_timeout_secs = protocol === 'udp' ? parseInt(idleTimeout) || 30 : null;
    r.udp_source_mode = protocol === 'udp' ? udpSourceMode : null;
    if (tlsEnabled && protocol === 'tcp') {
      r.tls = { mode: tlsMode, ca_cert: tlsCaCert, ca_key: tlsCaKey };
    }
    if (exporterEnabled) {
      r.exporter = {
        kind: exporterKind,
        path: exporterKind === 'uds' ? exporterPath : null,
        addr: exporterKind === 'tcp' ? exporterAddr : null,
      };
    }
    return r;
  }

  // Build pipeline preset from form state (for switching to Pipeline mode)
  function buildPipelinePreset() {
    const nodes = [
      { id: 1, kind: 'source', config: { protocol, listen, idle_timeout_secs: idleTimeout, udp_source_mode: udpSourceMode } },
    ];
    const edges = [];
    let nextId = 2;
    let lastId = 1;

    if (tlsEnabled && protocol === 'tcp') {
      nodes.push({ id: nextId, kind: 'tls_terminate', config: { ca_cert: tlsCaCert, ca_key: tlsCaKey } });
      edges.push({ from: lastId, to: nextId });
      lastId = nextId++;
      if (tlsMode === 'mitm') {
        nodes.push({ id: nextId, kind: 'tls_initiate', config: { verify_certs: true } });
        edges.push({ from: lastId, to: nextId });
        lastId = nextId++;
      }
    }

    filters.filter(f => f.pattern.trim()).forEach(f => {
      nodes.push({ id: nextId, kind: 'filter', config: { ...f } });
      edges.push({ from: lastId, to: nextId });
      lastId = nextId++;
    });

    nodes.push({ id: nextId, kind: 'forward', config: { target, tcp_nodelay: true } });
    edges.push({ from: lastId, to: nextId });
    const fwdId = nextId++;

    if (duplicate.trim()) {
      nodes.push({ id: nextId, kind: 'duplicate', config: { target: duplicate } });
      edges.push({ from: lastId, to: nextId });
      nextId++;
    }

    return { name: name || 'untitled', nodes, edges };
  }

  async function handleSave() {
    error = '';
    if (!name.trim()) { error = 'Rule name is required'; return; }
    if (!listen.trim()) { error = 'Listen address is required'; return; }
    if (!target.trim()) { error = 'Target address is required'; return; }

    saving = true;
    try {
      const r = buildRule();
      const result = isEditing
        ? await updateRule(rule.name, r)
        : await createRule(r);
      if (result.ok) {
        onSave?.();
      } else {
        error = result.error || 'Save failed';
      }
    } catch (e) {
      error = 'Save failed: ' + e.message;
    }
    saving = false;
  }

  // Pipeline mode preset for PipelineEditor
  let pipelinePreset = $derived(buildPipelinePreset());

  // Summary data
  let summaryRule = $derived(buildRule());
</script>

<div class="editor-page">
  <!-- Mode Tabs -->
  <div class="mode-tabs">
    <button class="mode-tab" class:active={mode === 'simple'} onclick={() => mode = 'simple'}>Simple</button>
    <button class="mode-tab" class:active={mode === 'pipeline'} onclick={() => mode = 'pipeline'}>Pipeline</button>
    <button class="mode-tab" class:active={mode === 'summary'} onclick={() => mode = 'summary'}>Summary</button>
    <div class="mode-tabs-spacer"></div>
    {#if mode === 'simple'}
      <button class="btn btn-accent" onclick={handleSave} disabled={saving}>
        {saving ? 'Saving...' : isEditing ? 'Update & Apply' : 'Save & Apply'}
      </button>
      <button class="btn" onclick={onCancel}>Cancel</button>
    {/if}
  </div>

  {#if error}
    <div class="form-error">{error}</div>
  {/if}

  {#if mode === 'simple'}
    <div class="simple-layout">
      <!-- Form -->
      <div class="form-sections">

        <!-- General -->
        <div class="form-section">
          <div class="form-section-title">General</div>
          <div class="form-row">
            <div class="form-field">
              <label for="rf-name">Rule Name</label>
              <input id="rf-name" class="mono" bind:value={name} placeholder="my-proxy-rule" disabled={isEditing}>
            </div>
            <div class="form-field">
              <label for="rf-proto">Protocol</label>
              <select id="rf-proto" bind:value={protocol}>
                <option value="tcp">TCP</option>
                <option value="udp">UDP</option>
              </select>
            </div>
          </div>
        </div>

        <!-- Endpoints -->
        <div class="form-section">
          <div class="form-section-title">Endpoints</div>
          <div class="form-row">
            <div class="form-field">
              <label for="rf-listen">Listen Address</label>
              <input id="rf-listen" class="mono" bind:value={listen} placeholder="0.0.0.0:8080">
              <span class="form-hint">ip:port to bind the listener</span>
            </div>
            <div class="form-field">
              <label for="rf-target">Target Address</label>
              <input id="rf-target" class="mono" bind:value={target} placeholder="10.0.0.1:80">
              <span class="form-hint">upstream ip:port to forward traffic</span>
            </div>
          </div>
        </div>

        <!-- TLS (TCP only) -->
        {#if protocol === 'tcp'}
          <div class="form-section">
            <div class="form-section-toggle">
              <label class="toggle-label">
                <input type="checkbox" bind:checked={tlsEnabled}>
                TLS Settings
              </label>
            </div>
            {#if tlsEnabled}
              <div class="form-row">
                <div class="form-field">
                  <label for="rf-tls-mode">TLS Mode</label>
                  <select id="rf-tls-mode" bind:value={tlsMode}>
                    <option value="passthrough">Passthrough</option>
                    <option value="terminate">Terminate</option>
                    <option value="mitm">MITM (terminate + re-encrypt)</option>
                  </select>
                </div>
              </div>
              {#if tlsMode !== 'passthrough'}
                <div class="form-row">
                  <div class="form-field">
                    <label for="rf-ca-cert">CA Certificate Path</label>
                    <input id="rf-ca-cert" class="mono" bind:value={tlsCaCert} placeholder="/etc/harpoon/ca.pem">
                  </div>
                  <div class="form-field">
                    <label for="rf-ca-key">CA Key Path</label>
                    <input id="rf-ca-key" class="mono" bind:value={tlsCaKey} placeholder="/etc/harpoon/ca-key.pem">
                  </div>
                </div>
              {/if}
            {/if}
          </div>
        {/if}

        <!-- Filters -->
        <div class="form-section">
          <div class="form-section-title">
            Filters
            <button class="btn btn-sm" style="margin-left:8px;" onclick={addFilter}>+ Add Filter</button>
          </div>
          {#if filters.length === 0}
            <div class="empty-hint">No filters configured. Traffic passes through unmodified.</div>
          {/if}
          {#each filters as filter, i}
            <div class="filter-row">
              <select bind:value={filter.kind}>
                <option value="substr">substr</option>
                <option value="bsubstr">bsubstr (hex)</option>
                <option value="regex">regex</option>
              </select>
              <select bind:value={filter.direction}>
                <option value="both">both</option>
                <option value="c2s">c→s</option>
                <option value="s2c">s→c</option>
              </select>
              <input class="mono" bind:value={filter.pattern} placeholder="pattern">
              <select bind:value={filter.action}>
                <option value="drop">drop</option>
                <option value="pass">pass</option>
                <option value="tap-only">tap-only</option>
              </select>
              <div class="filter-actions">
                <button class="btn-icon" onclick={() => moveFilter(i, -1)} disabled={i === 0}>↑</button>
                <button class="btn-icon" onclick={() => moveFilter(i, 1)} disabled={i === filters.length - 1}>↓</button>
                <button class="btn-icon btn-icon-danger" onclick={() => removeFilter(i)}>×</button>
              </div>
            </div>
          {/each}
        </div>

        <!-- Duplication & Export -->
        <div class="form-section">
          <div class="form-section-title">Duplication & Export</div>
          <div class="form-row">
            <div class="form-field">
              <label for="rf-dup">Duplicate To</label>
              <input id="rf-dup" class="mono" bind:value={duplicate} placeholder="10.0.0.2:9090 (optional)">
              <span class="form-hint">Send a copy of traffic to this endpoint</span>
            </div>
          </div>
          <div class="form-section-toggle" style="margin-top:12px;">
            <label class="toggle-label">
              <input type="checkbox" bind:checked={exporterEnabled}>
              Enable Exporter
            </label>
          </div>
          {#if exporterEnabled}
            <div class="form-row">
              <div class="form-field">
                <label for="rf-exp-kind">Exporter Type</label>
                <select id="rf-exp-kind" bind:value={exporterKind}>
                  <option value="tcp">TCP Framed</option>
                  <option value="uds">Unix Domain Socket</option>
                </select>
              </div>
              <div class="form-field">
                {#if exporterKind === 'tcp'}
                  <label for="rf-exp-addr">Exporter Address</label>
                  <input id="rf-exp-addr" class="mono" bind:value={exporterAddr} placeholder="127.0.0.1:4000">
                {:else}
                  <label for="rf-exp-path">Socket Path</label>
                  <input id="rf-exp-path" class="mono" bind:value={exporterPath} placeholder="/tmp/harpoon-export.sock">
                {/if}
              </div>
            </div>
          {/if}
        </div>

        <!-- UDP Settings -->
        {#if protocol === 'udp'}
          <div class="form-section">
            <div class="form-section-title">UDP Settings</div>
            <div class="form-row">
              <div class="form-field">
                <label for="rf-idle">Idle Timeout (seconds)</label>
                <input id="rf-idle" type="number" bind:value={idleTimeout}>
                <span class="form-hint">Sessions expire after this idle period</span>
              </div>
              <div class="form-field">
                <label for="rf-src-mode">Source Mode</label>
                <select id="rf-src-mode" bind:value={udpSourceMode}>
                  <option value="proxy">Proxy (upstream sees Harpoon IP)</option>
                  <option value="preserve">Preserve (upstream sees client IP, requires CAP_NET_ADMIN)</option>
                </select>
              </div>
            </div>
          </div>
        {/if}
      </div>

      <!-- Sticky Summary -->
      <div class="sticky-summary">
        <div class="summary-title">Summary</div>
        <div class="summary-row">
          <span class="summary-label">Protocol</span>
          <span class="badge" class:badge-tcp={protocol==='tcp'} class:badge-udp={protocol==='udp'}>{protocol}</span>
        </div>
        <div class="summary-row">
          <span class="summary-label">Listen</span>
          <span class="mono">{listen || '—'}</span>
        </div>
        <div class="summary-row">
          <span class="summary-label">Target</span>
          <span class="mono">{target || '—'}</span>
        </div>
        {#if tlsEnabled && protocol === 'tcp'}
          <div class="summary-row">
            <span class="summary-label">TLS</span>
            <span class="badge badge-ok">{tlsMode}</span>
          </div>
        {/if}
        <div class="summary-row">
          <span class="summary-label">Filters</span>
          <span>{filters.filter(f => f.pattern.trim()).length}</span>
        </div>
        {#if duplicate.trim()}
          <div class="summary-row">
            <span class="summary-label">Duplicate</span>
            <span class="mono">{duplicate}</span>
          </div>
        {/if}
        {#if exporterEnabled}
          <div class="summary-row">
            <span class="summary-label">Exporter</span>
            <span class="badge badge-ok">{exporterKind}</span>
          </div>
        {/if}
      </div>
    </div>

  {:else if mode === 'pipeline'}
    <PipelineEditor
      preset={pipelinePreset}
      onSave={onSave}
      onCancel={() => mode = 'simple'}
    />

  {:else if mode === 'summary'}
    <div class="summary-view">
      <div class="form-section">
        <div class="form-section-title">Effective Configuration</div>
        <pre class="code-block">{JSON.stringify(summaryRule, null, 2)}</pre>
      </div>
      <div class="form-section">
        <div class="form-section-title">Pipeline Representation</div>
        <p class="summary-hint">This rule compiles to a <strong>{
          !filters.some(f => f.pattern.trim()) && !duplicate.trim() && !tlsEnabled && !exporterEnabled
            ? 'FastForward (Tier 0 — zero copy)'
            : 'Linear (Tier 1 — sequential processing)'
        }</strong> execution plan.</p>
        <div class="summary-nodes">
          <span class="summary-node" style="border-color: var(--accent);">Source</span>
          <span class="summary-arrow">→</span>
          {#if tlsEnabled && protocol === 'tcp'}
            <span class="summary-node" style="border-color: var(--info);">TLS {tlsMode}</span>
            <span class="summary-arrow">→</span>
          {/if}
          {#if filters.some(f => f.pattern.trim())}
            <span class="summary-node" style="border-color: var(--warn);">Filter ({filters.filter(f => f.pattern.trim()).length})</span>
            <span class="summary-arrow">→</span>
          {/if}
          <span class="summary-node" style="border-color: var(--ok);">Forward</span>
          {#if duplicate.trim()}
            <span class="summary-arrow">+</span>
            <span class="summary-node" style="border-color: var(--accent-2);">Duplicate</span>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .editor-page { display: flex; flex-direction: column; gap: 0; }

  .mode-tabs {
    display: flex; align-items: center; gap: 0; margin-bottom: 20px;
    border-bottom: 1px solid var(--bg-4); padding-bottom: 0;
  }
  .mode-tab {
    padding: 10px 20px; background: none; border: none; border-bottom: 2px solid transparent;
    color: var(--text-3); font-size: 13px; font-weight: 500; cursor: pointer;
    font-family: var(--font); transition: all .15s;
  }
  .mode-tab:hover { color: var(--text); }
  .mode-tab.active { color: var(--accent); border-bottom-color: var(--accent); }
  .mode-tabs-spacer { flex: 1; }

  .form-error {
    background: rgba(226,109,109,.1); border: 1px solid var(--err); border-radius: var(--radius-sm);
    padding: 10px 14px; color: var(--err); font-size: 12px; margin-bottom: 16px;
  }

  .simple-layout { display: grid; grid-template-columns: 1fr 240px; gap: 24px; }
  @media (max-width: 900px) { .simple-layout { grid-template-columns: 1fr; } }

  .form-sections { display: flex; flex-direction: column; gap: 0; }
  .form-section {
    padding: 20px; background: var(--bg-2); border: 1px solid var(--bg-4);
    border-radius: var(--radius); margin-bottom: 16px;
  }
  .form-section-title {
    font-size: 13px; font-weight: 600; color: var(--text); margin-bottom: 14px;
    display: flex; align-items: center;
  }
  .form-section-toggle { margin-bottom: 12px; }
  .toggle-label {
    display: flex; align-items: center; gap: 8px; font-size: 13px; font-weight: 500;
    color: var(--text-2); cursor: pointer; text-transform: none; letter-spacing: 0;
  }
  .toggle-label input[type="checkbox"] { width: auto; accent-color: var(--accent); }

  .form-row { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-bottom: 12px; }
  .form-field { display: flex; flex-direction: column; gap: 4px; }
  .form-hint { font-size: 11px; color: var(--text-4); }
  .empty-hint { font-size: 12px; color: var(--text-4); padding: 8px 0; }

  .filter-row {
    display: grid; grid-template-columns: 100px 70px 1fr 80px auto; gap: 6px;
    margin-bottom: 6px; align-items: center;
  }
  .filter-row select, .filter-row input { font-size: 12px; padding: 6px 8px; }
  .filter-actions { display: flex; gap: 2px; }
  .btn-icon {
    width: 26px; height: 26px; display: flex; align-items: center; justify-content: center;
    background: var(--bg-3); border: 1px solid var(--bg-4); border-radius: 6px;
    color: var(--text-3); cursor: pointer; font-size: 14px; font-family: var(--font);
  }
  .btn-icon:hover { background: var(--bg-4); color: var(--text); }
  .btn-icon:disabled { opacity: .3; cursor: default; }
  .btn-icon-danger:hover { color: var(--err); }

  .sticky-summary {
    position: sticky; top: 24px; background: var(--bg-2); border: 1px solid var(--bg-4);
    border-radius: var(--radius); padding: 16px; height: fit-content;
  }
  .summary-title { font-size: 12px; font-weight: 600; color: var(--text-3); text-transform: uppercase; letter-spacing: .5px; margin-bottom: 12px; }
  .summary-row { display: flex; justify-content: space-between; align-items: center; padding: 4px 0; font-size: 12px; color: var(--text-2); }
  .summary-label { color: var(--text-3); }

  .summary-view { max-width: 700px; }
  .code-block {
    background: var(--bg-1); border: 1px solid var(--bg-4); border-radius: var(--radius-sm);
    padding: 16px; font-family: var(--mono); font-size: 12px; color: var(--text-2);
    overflow-x: auto; white-space: pre; line-height: 1.6;
  }
  .summary-hint { font-size: 13px; color: var(--text-3); margin-bottom: 16px; }
  .summary-nodes { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .summary-node {
    padding: 6px 14px; border: 1px solid; border-radius: var(--radius-sm);
    font-size: 12px; font-weight: 500; color: var(--text); background: var(--bg-3);
  }
  .summary-arrow { color: var(--text-4); font-size: 16px; }
</style>
