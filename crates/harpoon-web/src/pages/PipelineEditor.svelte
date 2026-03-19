<script>
  import { NODE_KINDS, CATEGORIES, PRESETS } from '../lib/types.js';
  import { createRule, updateRule, validatePipeline, createPipeline } from '../lib/api.js';
  import NodeBlock from '../components/NodeBlock.svelte';

  let { preset = null, onSave, onCancel } = $props();

  // Pipeline state
  let pipelineName = $state(preset?.name || '');
  let nodes = $state(preset?.nodes?.map(n => ({
    ...n, x: 0, y: 0, config: { ...n.config },
  })) || []);
  let edges = $state(preset?.edges?.map((e, i) => ({ id: i + 1, ...e })) || []);
  let nextId = $state(Math.max(0, ...nodes.map(n => n.id)) + 1);
  let selectedNode = $state(null);
  let mode = $state(preset ? 'editor' : 'presets'); // presets | editor
  let error = $state('');
  let validationResult = $state(null);

  // Auto-layout nodes vertically
  $effect(() => {
    if (nodes.length > 0 && nodes[0].x === 0) layoutNodes();
  });

  function layoutNodes() {
    // Simple top-down layout by topological order
    const placed = new Set();
    const levels = {};
    function assignLevel(id, level) {
      if (placed.has(id)) return;
      placed.add(id);
      levels[id] = Math.max(levels[id] || 0, level);
      edges.filter(e => e.from === id).forEach(e => assignLevel(e.to, level + 1));
    }
    // Start from source nodes
    const sourceIds = nodes.filter(n => NODE_KINDS[n.kind]?.category === 'source').map(n => n.id);
    sourceIds.forEach(id => assignLevel(id, 0));
    nodes.filter(n => !placed.has(n.id)).forEach(n => assignLevel(n.id, Object.keys(levels).length));

    // Group by level
    const byLevel = {};
    for (const [id, level] of Object.entries(levels)) {
      if (!byLevel[level]) byLevel[level] = [];
      byLevel[level].push(parseInt(id));
    }

    const NODE_W = 220, NODE_H = 80, GAP_X = 40, GAP_Y = 100;
    const startX = 400;
    for (const [level, ids] of Object.entries(byLevel)) {
      const totalWidth = ids.length * NODE_W + (ids.length - 1) * GAP_X;
      let x = startX - totalWidth / 2;
      for (const id of ids) {
        const node = nodes.find(n => n.id === id);
        if (node) {
          node.x = x;
          node.y = parseInt(level) * (NODE_H + GAP_Y) + 40;
          x += NODE_W + GAP_X;
        }
      }
    }
    nodes = [...nodes]; // trigger reactivity
  }

  function selectPreset(presetDef) {
    pipelineName = presetDef.name;
    nodes = presetDef.nodes.map(n => ({ ...n, x: 0, y: 0, config: { ...n.config } }));
    edges = presetDef.edges.map((e, i) => ({ id: i + 1, ...e }));
    nextId = Math.max(0, ...nodes.map(n => n.id)) + 1;
    mode = 'editor';
    layoutNodes();
  }

  function addNode(kindKey) {
    const kind = NODE_KINDS[kindKey];
    if (!kind) return;
    const config = {};
    kind.fields.forEach(f => { config[f.key] = f.default || ''; });
    const id = nextId++;
    nodes = [...nodes, { id, kind: kindKey, x: 300, y: nodes.length * 120 + 40, config }];
    selectedNode = id;
  }

  function removeNode(id) {
    nodes = nodes.filter(n => n.id !== id);
    edges = edges.filter(e => e.from !== id && e.to !== id);
    if (selectedNode === id) selectedNode = null;
  }

  function addEdge(fromId, toId, port) {
    if (fromId === toId) return;
    if (edges.some(e => e.from === fromId && e.to === toId)) return;
    edges = [...edges, { id: Date.now(), from: fromId, to: toId, port: port || null }];
  }

  function removeEdge(id) {
    edges = edges.filter(e => e.id !== id);
  }

  // Dragging
  let dragging = $state(null);
  let connecting = $state(null); // { fromId, startX, startY, curX, curY }

  function onNodeMouseDown(e, nodeId) {
    if (e.button !== 0) return;
    const node = nodes.find(n => n.id === nodeId);
    if (!node) return;
    dragging = { id: nodeId, offsetX: e.clientX - node.x, offsetY: e.clientY - node.y };
    selectedNode = nodeId;
  }

  function onCanvasMouseMove(e) {
    if (dragging) {
      const node = nodes.find(n => n.id === dragging.id);
      if (node) {
        node.x = e.clientX - dragging.offsetX;
        node.y = e.clientY - dragging.offsetY;
        nodes = [...nodes];
      }
    }
    if (connecting) {
      connecting = { ...connecting, curX: e.offsetX, curY: e.offsetY };
    }
  }

  function onCanvasMouseUp() {
    dragging = null;
    connecting = null;
  }

  function startConnect(nodeId, e) {
    const node = nodes.find(n => n.id === nodeId);
    if (!node) return;
    connecting = {
      fromId: nodeId,
      startX: node.x + 110, startY: node.y + 70,
      curX: node.x + 110, curY: node.y + 90,
    };
  }

  function endConnect(nodeId) {
    if (connecting && connecting.fromId !== nodeId) {
      addEdge(connecting.fromId, nodeId);
    }
    connecting = null;
  }

  // Build pipeline JSON for API
  function buildPipelinePayload() {
    return {
      name: pipelineName,
      nodes: nodes.map(n => ({
        id: n.id,
        kind: n.kind,
        label: NODE_KINDS[n.kind]?.label || n.kind,
        config: n.config,
        x: n.x, y: n.y,
      })),
      edges: edges.map(e => ({ from: e.from, to: e.to, port: e.port })),
    };
  }

  // Convert pipeline to Rule for current backend compatibility
  function pipelineToRule() {
    const sourceNode = nodes.find(n => n.kind === 'source');
    const forwardNode = nodes.find(n => n.kind === 'forward');
    const filterNodes = nodes.filter(n => n.kind === 'filter');
    const dupNode = nodes.find(n => n.kind === 'duplicate');
    const exportNode = nodes.find(n => n.kind === 'export');
    const tlsTermNode = nodes.find(n => n.kind === 'tls_terminate');
    const tlsInitNode = nodes.find(n => n.kind === 'tls_initiate');

    if (!sourceNode || !forwardNode) return null;

    const rule = {
      name: pipelineName,
      protocol: sourceNode.config.protocol || 'tcp',
      listen: sourceNode.config.listen || '',
      target: forwardNode.config.target || '',
      filters: filterNodes.map(f => ({
        kind: f.config.kind || 'substr',
        pattern: f.config.pattern || '',
        direction: f.config.direction || 'both',
        action: f.config.action || 'drop',
      })).filter(f => f.pattern),
      duplicate: dupNode?.config?.target || null,
      idle_timeout_secs: sourceNode.config.protocol === 'udp' ? parseInt(sourceNode.config.idle_timeout_secs) || 30 : null,
      udp_source_mode: sourceNode.config.protocol === 'udp' ? sourceNode.config.udp_source_mode : null,
    };

    if (tlsTermNode) {
      rule.tls = {
        mode: tlsInitNode ? 'mitm' : 'terminate',
        ca_cert: tlsTermNode.config.ca_cert || '',
        ca_key: tlsTermNode.config.ca_key || '',
      };
    }

    if (exportNode) {
      rule.exporter = {
        kind: exportNode.config.kind || 'tcp',
        path: exportNode.config.kind === 'uds' ? exportNode.config.path : null,
        addr: exportNode.config.kind === 'tcp' ? exportNode.config.addr : null,
      };
    }

    return rule;
  }

  async function handleValidate() {
    error = '';
    const payload = buildPipelinePayload();
    // For now validate by trying to convert to rule
    const rule = pipelineToRule();
    if (!rule) {
      error = 'Pipeline must have at least a Source and a Forward node';
      return;
    }
    if (!rule.listen || !rule.target) {
      error = 'Source listen address and Forward target address are required';
      return;
    }
    validationResult = { ok: true, message: 'Pipeline is valid' };
  }

  async function handleSave() {
    error = '';
    if (!pipelineName) { error = 'Pipeline name is required'; return; }

    // Try saving as a pipeline first
    const payload = buildPipelinePayload();
    const appPipeline = {
      id: pipelineName.toLowerCase().replace(/\s+/g, '-'),
      name: pipelineName,
      nodes: payload.nodes.map(n => ({
        id: n.id,
        kind: n.kind,
        label: n.label,
        config: n.config || {},
      })),
      edges: payload.edges,
    };

    try {
      // Try pipeline API
      const result = await createPipeline(appPipeline);
      if (result.ok) {
        onSave?.();
        return;
      }
      // If pipeline API fails, fall back to rule conversion
      const rule = pipelineToRule();
      if (rule && rule.listen && rule.target) {
        const ruleResult = await createRule(rule);
        if (ruleResult.ok) {
          onSave?.();
          return;
        }
        error = ruleResult.error || result.error || 'Save failed';
      } else {
        error = result.error || 'Pipeline must have valid Source and Forward nodes';
      }
    } catch (e) {
      error = 'Save failed: ' + e.message;
    }
  }

  // Derived: SVG edge paths
  let edgePaths = $derived(edges.map(e => {
    const from = nodes.find(n => n.id === e.from);
    const to = nodes.find(n => n.id === e.to);
    if (!from || !to) return null;
    const x1 = from.x + 110, y1 = from.y + 70;
    const x2 = to.x + 110, y2 = to.y;
    const cy = (y1 + y2) / 2;
    return { id: e.id, path: `M${x1},${y1} C${x1},${cy} ${x2},${cy} ${x2},${y2}`, from: e.from, to: e.to };
  }).filter(Boolean));

  let selectedNodeData = $derived(selectedNode ? nodes.find(n => n.id === selectedNode) : null);
  let selectedKindDef = $derived(selectedNodeData ? NODE_KINDS[selectedNodeData.kind] : null);
</script>

{#if mode === 'presets'}
  <div class="presets-page">
    <div class="section-title">Create Pipeline</div>
    <p style="color: var(--text-3); margin-bottom: 24px;">Choose a preset template or start from scratch</p>
    <div class="preset-grid">
      {#each PRESETS as p}
        <button class="preset-card" onclick={() => selectPreset(p)}>
          <div class="preset-name">{p.name}</div>
          <div class="preset-desc">{p.description}</div>
          <div class="preset-meta">{p.nodes.length} nodes</div>
        </button>
      {/each}
      <button class="preset-card" onclick={() => { mode = 'editor'; }}>
        <div class="preset-name">Blank Pipeline</div>
        <div class="preset-desc">Start from scratch</div>
        <div class="preset-meta">Empty canvas</div>
      </button>
    </div>
  </div>
{:else}
  <div class="editor-layout">
    <!-- Toolbar -->
    <div class="editor-toolbar">
      <input class="pipeline-name-input" bind:value={pipelineName} placeholder="Pipeline name" />
      <button class="btn" onclick={handleValidate}>Validate</button>
      <button class="btn btn-accent" onclick={handleSave}>Save & Apply</button>
      <button class="btn" onclick={() => { mode = 'presets'; }}>Presets</button>
      <button class="btn" onclick={layoutNodes}>Auto Layout</button>
      <button class="btn btn-danger" onclick={onCancel}>Cancel</button>
      {#if error}
        <span class="toolbar-error">{error}</span>
      {/if}
      {#if validationResult?.ok}
        <span class="toolbar-ok">{validationResult.message}</span>
      {/if}
    </div>

    <!-- Palette -->
    <div class="palette">
      <div class="palette-title">Nodes</div>
      {#each Object.entries(CATEGORIES) as [catKey, cat]}
        <div class="palette-cat" style="border-left-color: {cat.color}">{cat.label}</div>
        {#each Object.entries(NODE_KINDS).filter(([_, v]) => v.category === catKey) as [kindKey, kindDef]}
          <button class="palette-item" onclick={() => addNode(kindKey)}>
            <span class="palette-dot" style="background: {kindDef.color}"></span>
            {kindDef.label}
          </button>
        {/each}
      {/each}
    </div>

    <!-- Canvas -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="canvas"
         onmousemove={onCanvasMouseMove}
         onmouseup={onCanvasMouseUp}>
      <svg class="edges-svg">
        {#each edgePaths as ep}
          <path d={ep.path} stroke="var(--bg-4)" stroke-width="2" fill="none" />
          <path d={ep.path} stroke="var(--accent)" stroke-width="2" fill="none" opacity="0.3" />
        {/each}
        {#if connecting}
          <line x1={connecting.startX} y1={connecting.startY}
                x2={connecting.curX} y2={connecting.curY}
                stroke="var(--accent)" stroke-width="2" stroke-dasharray="6" />
        {/if}
      </svg>

      {#each nodes as node (node.id)}
        <NodeBlock
          {node}
          kindDef={NODE_KINDS[node.kind]}
          selected={selectedNode === node.id}
          onMouseDown={(e) => onNodeMouseDown(e, node.id)}
          onConnectStart={(e) => startConnect(node.id, e)}
          onConnectEnd={() => endConnect(node.id)}
          onRemove={() => removeNode(node.id)}
        />
      {/each}
    </div>

    <!-- Inspector -->
    <div class="inspector">
      {#if selectedNodeData && selectedKindDef}
        <div class="inspector-title" style="color: {selectedKindDef.color}">
          {selectedKindDef.label}
        </div>
        <div class="inspector-desc">{selectedKindDef.description}</div>

        {#each selectedKindDef.fields as field}
          <div class="inspector-field">
            <label>{field.key}</label>
            {#if field.type === 'select'}
              <select bind:value={selectedNodeData.config[field.key]}>
                {#each field.options as opt}
                  <option value={opt}>{opt}</option>
                {/each}
              </select>
            {:else if field.type === 'bool'}
              <label class="checkbox-label">
                <input type="checkbox" bind:checked={selectedNodeData.config[field.key]} />
                {field.key}
              </label>
            {:else if field.type === 'number'}
              <input type="number" bind:value={selectedNodeData.config[field.key]} />
            {:else}
              <input type="text" class:mono={field.mono}
                     placeholder={field.placeholder || ''}
                     bind:value={selectedNodeData.config[field.key]} />
            {/if}
          </div>
        {/each}

        <div style="margin-top: 16px;">
          <div class="inspector-desc">Connections</div>
          <div style="font-size: 11px; color: var(--text-3); margin-top: 4px;">
            In: {edges.filter(e => e.to === selectedNode).length} |
            Out: {edges.filter(e => e.from === selectedNode).length}
          </div>
        </div>

        <button class="btn btn-danger btn-sm" style="margin-top: 12px;" onclick={() => removeNode(selectedNode)}>
          Remove Node
        </button>
      {:else}
        <div class="inspector-empty">Select a node to inspect</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .presets-page { max-width: 800px; }
  .preset-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 16px; }
  .preset-card {
    background: var(--bg-2); border: 1px solid var(--bg-4); border-radius: var(--radius);
    padding: 20px; cursor: pointer; text-align: left; transition: all .15s; color: var(--text);
    font-family: var(--font);
  }
  .preset-card:hover { border-color: var(--accent); background: var(--bg-3); }
  .preset-name { font-size: 14px; font-weight: 600; margin-bottom: 4px; }
  .preset-desc { font-size: 12px; color: var(--text-3); margin-bottom: 8px; }
  .preset-meta { font-size: 11px; color: var(--text-4); font-family: var(--mono); }

  .editor-layout {
    display: grid;
    grid-template-columns: 200px 1fr 260px;
    grid-template-rows: 48px 1fr;
    height: calc(100vh - var(--topbar-h) - 48px);
    gap: 0;
    margin: -24px;
  }

  .editor-toolbar {
    grid-column: 1 / 4; display: flex; align-items: center; gap: 8px;
    padding: 0 16px; background: var(--bg-1); border-bottom: 1px solid var(--bg-4);
  }
  .pipeline-name-input {
    width: 200px; padding: 6px 12px; background: var(--bg-2); border: 1px solid var(--bg-4);
    border-radius: var(--radius-sm); color: var(--accent); font-family: var(--mono);
    font-size: 14px; font-weight: 600;
  }
  .toolbar-error { color: var(--err); font-size: 12px; margin-left: 8px; }
  .toolbar-ok { color: var(--ok); font-size: 12px; margin-left: 8px; }

  .palette {
    background: var(--bg-1); border-right: 1px solid var(--bg-4);
    overflow-y: auto; padding: 12px;
  }
  .palette-title { font-size: 12px; font-weight: 600; color: var(--text-3); margin-bottom: 12px; text-transform: uppercase; letter-spacing: .5px; }
  .palette-cat { font-size: 11px; color: var(--text-3); padding: 4px 0 4px 8px; border-left: 2px solid; margin-top: 8px; }
  .palette-item {
    display: flex; align-items: center; gap: 8px; width: 100%; padding: 6px 8px;
    background: none; border: 1px solid transparent; border-radius: var(--radius-sm);
    color: var(--text-2); font-size: 12px; cursor: pointer; font-family: var(--font); text-align: left;
  }
  .palette-item:hover { background: var(--bg-2); border-color: var(--bg-4); }
  .palette-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }

  .canvas {
    position: relative; background: var(--bg-0); overflow: hidden; cursor: default;
    background-image: radial-gradient(circle, var(--bg-4) 1px, transparent 1px);
    background-size: 24px 24px;
  }
  .edges-svg { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }

  .inspector {
    background: var(--bg-1); border-left: 1px solid var(--bg-4);
    overflow-y: auto; padding: 16px;
  }
  .inspector-title { font-size: 16px; font-weight: 600; margin-bottom: 4px; }
  .inspector-desc { font-size: 12px; color: var(--text-3); margin-bottom: 16px; }
  .inspector-field { margin-bottom: 12px; }
  .inspector-empty { color: var(--text-4); font-size: 13px; text-align: center; margin-top: 40px; }
  .checkbox-label { display: flex; align-items: center; gap: 8px; font-size: 13px; color: var(--text-2); cursor: pointer; text-transform: none; letter-spacing: 0; }
  .checkbox-label input[type="checkbox"] { width: auto; }
</style>
