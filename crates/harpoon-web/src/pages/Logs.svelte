<script>
  let { data } = $props();

  function fmtBytes(n) { if(!n) return '0 B'; if(n<1024) return n+' B'; if(n<1048576) return (n/1024).toFixed(1)+' KB'; return (n/1048576).toFixed(1)+' MB'; }
  function fmtTime(ms) { if(!ms) return '\u2014'; const d=new Date(ms); return d.toLocaleTimeString('en-GB',{hour12:false})+'.'+String(ms%1000).padStart(3,'0'); }

  let events = $derived(data?.events ?? []);

  let paused = $state(false);
  let levelFilter = $state('all');
  let searchText = $state('');
  let frozenEvents = $state([]);

  let logContainer = $state(null);

  function togglePause() {
    if (!paused) {
      frozenEvents = [...events];
    }
    paused = !paused;
  }

  let activeEvents = $derived(paused ? frozenEvents : events);

  function classifyLevel(ev) {
    const kind = (ev.kind ?? '').toLowerCase();
    if (kind === 'error') return 'ERROR';
    if (kind === 'warning' || kind === 'drop') return 'WARN';
    return 'INFO';
  }

  function classifyComponent(ev) {
    const kind = (ev.kind ?? '').toLowerCase();
    if (kind === 'connection' || kind === 'tcp') return 'tcp';
    if (kind === 'udp') return 'udp';
    if (kind === 'filter' || kind === 'drop') return 'filter';
    if (kind === 'export') return 'export';
    if (kind === 'reload' || kind === 'start' || kind === 'stop') return 'engine';
    return 'system';
  }

  let filteredEvents = $derived(() => {
    let result = [...activeEvents];

    // Sort oldest first for log view (chronological)
    result.sort((a, b) => (a.timestamp_ms ?? 0) - (b.timestamp_ms ?? 0));

    // Level filter
    if (levelFilter !== 'all') {
      result = result.filter(ev => classifyLevel(ev) === levelFilter);
    }

    // Text search
    if (searchText.trim()) {
      const q = searchText.trim().toLowerCase();
      result = result.filter(ev =>
        (ev.detail ?? '').toLowerCase().includes(q) ||
        (ev.kind ?? '').toLowerCase().includes(q) ||
        classifyComponent(ev).toLowerCase().includes(q)
      );
    }

    return result;
  });

  function levelClass(level) {
    if (level === 'ERROR') return 'level-error';
    if (level === 'WARN') return 'level-warn';
    return 'level-info';
  }

  // Auto-scroll to bottom when new events arrive (unless paused)
  $effect(() => {
    const evts = filteredEvents();
    if (!paused && logContainer) {
      // Use tick-like delay to scroll after DOM update
      requestAnimationFrame(() => {
        if (logContainer) {
          logContainer.scrollTop = logContainer.scrollHeight;
        }
      });
    }
  });
</script>

<div class="logs-page">
  <div class="page-header">
    <h1 class="page-title">Logs</h1>
    <span class="page-count mono">{filteredEvents().length} / {activeEvents.length} entries</span>
  </div>

  <!-- Controls -->
  <div class="controls-row">
    <div class="control-group">
      <label>Level</label>
      <select bind:value={levelFilter}>
        <option value="all">All</option>
        <option value="ERROR">Error</option>
        <option value="WARN">Warning</option>
        <option value="INFO">Info</option>
      </select>
    </div>
    <div class="control-group search-group">
      <label>Search</label>
      <input type="text" placeholder="Filter logs..." bind:value={searchText} class="mono" />
    </div>
    <div class="control-group">
      <label>&nbsp;</label>
      <button class="btn" class:btn-active={paused} onclick={togglePause}>
        {paused ? 'Resume' : 'Pause'}
      </button>
    </div>
    {#if paused}
      <div class="pause-indicator">
        <span class="pause-dot"></span>
        Paused
      </div>
    {/if}
  </div>

  <!-- Log Stream -->
  <div class="card log-card">
    {#if filteredEvents().length === 0}
      <div class="empty-state">
        {#if activeEvents.length === 0}
          No log events recorded yet.
        {:else}
          No log entries match your filters.
        {/if}
      </div>
    {:else}
      <div class="log-stream" bind:this={logContainer}>
        <pre class="log-pre mono">{#each filteredEvents() as ev}{@const level = classifyLevel(ev)}{@const comp = classifyComponent(ev)}<span class="log-line"><span class="log-time">{fmtTime(ev.timestamp_ms)}</span> <span class={levelClass(level)}>{level.padEnd(5)}</span> <span class="log-component">[{comp}]</span> <span class="log-msg">{ev.detail ?? ''}</span>
</span>{/each}</pre>
      </div>
    {/if}
  </div>
</div>

<style>
  .logs-page { display: flex; flex-direction: column; gap: 16px; }

  .page-header { display: flex; align-items: baseline; gap: 12px; }
  .page-title { font-size: 22px; font-weight: 700; color: var(--text); }
  .page-count { font-size: 12px; color: var(--text-3); }

  .controls-row {
    display: flex; align-items: flex-end; gap: 12px; flex-wrap: wrap;
  }
  .control-group { display: flex; flex-direction: column; gap: 4px; }
  .control-group select { width: 140px; }
  .search-group { flex: 1; min-width: 200px; max-width: 400px; }

  .btn-active { background: var(--warn); color: var(--bg-0); border-color: var(--warn); font-weight: 600; }
  .btn-active:hover { background: var(--warn); opacity: 0.9; }

  .pause-indicator {
    display: flex; align-items: center; gap: 6px; font-size: 12px;
    color: var(--warn); font-weight: 600; padding-bottom: 8px;
  }
  .pause-dot {
    width: 8px; height: 8px; border-radius: 50%; background: var(--warn);
    animation: pulse 1.5s ease-in-out infinite;
  }
  @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }

  .log-card { padding: 0; overflow: hidden; }

  .log-stream {
    max-height: calc(100vh - 260px); overflow-y: auto; overflow-x: auto;
    padding: 12px 0;
  }

  .log-pre {
    margin: 0; font-size: 12px; line-height: 1.65; white-space: pre;
    padding: 0 16px;
  }

  .log-line { display: inline; }
  .log-time { color: var(--text-4); }
  .log-component { color: var(--info); }
  .log-msg { color: var(--text-2); }

  .level-error { color: var(--err); font-weight: 600; }
  .level-warn { color: var(--warn); font-weight: 600; }
  .level-info { color: var(--text-3); }

  .empty-state { color: var(--text-4); font-size: 13px; padding: 32px 0; text-align: center; }
</style>
