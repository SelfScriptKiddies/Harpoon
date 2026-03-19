<script>
  let { data } = $props();

  function fmtBytes(n) { if(!n) return '0 B'; if(n<1024) return n+' B'; if(n<1048576) return (n/1024).toFixed(1)+' KB'; if(n<1073741824) return (n/1048576).toFixed(1)+' MB'; return (n/1073741824).toFixed(2)+' GB'; }
  function fmtUptime(s) { if(!s) return '\u2014'; if(s<60) return s+'s'; if(s<3600) return Math.floor(s/60)+'m '+(s%60)+'s'; return Math.floor(s/3600)+'h '+Math.floor((s%3600)/60)+'m'; }
  function fmtTime(ms) { if(!ms) return '\u2014'; const d=new Date(ms); return d.toLocaleTimeString('en-GB',{hour12:false})+'.'+String(ms%1000).padStart(3,'0'); }

  let events = $derived(data?.events ?? []);

  let kindFilter = $state('all');
  let searchText = $state('');
  let paused = $state(false);

  // Snapshot events when paused
  let pausedSnapshot = $state([]);

  $effect(() => {
    if (!paused) {
      pausedSnapshot = [];
    }
  });

  function togglePause() {
    if (!paused) {
      // Capture current events before pausing
      pausedSnapshot = [...events];
    }
    paused = !paused;
  }

  let activeEvents = $derived(paused ? pausedSnapshot : events);

  let uniqueKinds = $derived(() => {
    const kinds = new Set(events.map(e => e.kind).filter(Boolean));
    return [...kinds].sort();
  });

  let filteredEvents = $derived(() => {
    let result = [...activeEvents];

    // Sort newest first
    result.sort((a, b) => (b.timestamp_ms ?? 0) - (a.timestamp_ms ?? 0));

    // Filter by kind
    if (kindFilter !== 'all') {
      result = result.filter(e => e.kind === kindFilter);
    }

    // Filter by search text
    if (searchText.trim()) {
      const q = searchText.trim().toLowerCase();
      result = result.filter(e =>
        (e.detail ?? '').toLowerCase().includes(q) ||
        (e.kind ?? '').toLowerCase().includes(q)
      );
    }

    return result;
  });
</script>

<div class="events-page">
  <div class="page-header">
    <h1 class="page-title">Events</h1>
    <span class="page-count mono">{filteredEvents().length} / {activeEvents.length} events</span>
  </div>

  <!-- Controls -->
  <div class="controls-row">
    <div class="control-group">
      <label>Kind</label>
      <select bind:value={kindFilter}>
        <option value="all">All kinds</option>
        {#each uniqueKinds() as kind}
          <option value={kind}>{kind}</option>
        {/each}
      </select>
    </div>
    <div class="control-group search-group">
      <label>Search</label>
      <input type="text" placeholder="Filter by text..." bind:value={searchText} />
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

  <!-- Events Table -->
  <div class="card table-section">
    {#if filteredEvents().length === 0}
      <div class="empty-state">
        {#if activeEvents.length === 0}
          No events recorded yet.
        {:else}
          No events match your filters.
        {/if}
      </div>
    {:else}
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th class="col-time">Time</th>
              <th class="col-kind">Kind</th>
              <th class="col-detail">Detail</th>
            </tr>
          </thead>
          <tbody>
            {#each filteredEvents() as ev}
              <tr>
                <td class="mono col-time">{fmtTime(ev.timestamp_ms)}</td>
                <td class="col-kind">
                  <span class="badge"
                    class:badge-ok={ev.kind === 'info' || ev.kind === 'start' || ev.kind === 'reload'}
                    class:badge-err={ev.kind === 'error'}
                    class:badge-warn={ev.kind === 'warning' || ev.kind === 'drop'}
                    class:badge-tcp={ev.kind === 'connection' || ev.kind === 'tcp'}
                    class:badge-udp={ev.kind === 'udp'}
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
  .events-page { display: flex; flex-direction: column; gap: 16px; }

  .page-header { display: flex; align-items: baseline; gap: 12px; }
  .page-title { font-size: 22px; font-weight: 700; color: var(--text); }
  .page-count { font-size: 12px; color: var(--text-3); }

  .controls-row {
    display: flex; align-items: flex-end; gap: 12px; flex-wrap: wrap;
  }
  .control-group { display: flex; flex-direction: column; gap: 4px; }
  .control-group select { width: 160px; }
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

  .table-section { overflow: hidden; }
  .table-wrap { overflow-x: auto; max-height: calc(100vh - 260px); overflow-y: auto; }

  table { width: 100%; border-collapse: collapse; font-size: 12px; }
  thead th {
    text-align: left; padding: 8px 10px; font-size: 10px; font-weight: 600;
    color: var(--text-3); text-transform: uppercase; letter-spacing: .5px;
    border-bottom: 1px solid var(--bg-4); white-space: nowrap;
    position: sticky; top: 0; background: var(--bg-2); z-index: 1;
  }
  tbody td {
    padding: 6px 10px; border-bottom: 1px solid var(--bg-3); color: var(--text-2);
  }
  tbody tr:hover { background: var(--bg-3); }
  tbody tr:last-child td { border-bottom: none; }

  .col-time { width: 130px; white-space: nowrap; }
  .col-kind { width: 100px; white-space: nowrap; }
  .detail-cell { white-space: normal; word-break: break-all; color: var(--text-2); }

  .empty-state { color: var(--text-4); font-size: 13px; padding: 20px 0; text-align: center; }
</style>
