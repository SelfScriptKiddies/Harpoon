<script>
  let { data, onRefresh, onLogout, onCreatePipeline } = $props();

  let isRunning = $derived(data?.status?.running ?? false);

  let totalTcp = $derived(
    Array.isArray(data?.stats)
      ? data.stats.reduce((sum, s) => sum + (s.tcp_connections ?? 0), 0)
      : 0
  );

  let totalUdp = $derived(
    Array.isArray(data?.stats)
      ? data.stats.reduce((sum, s) => sum + (s.udp_sessions ?? 0), 0)
      : 0
  );

  let rulesCount = $derived(
    Array.isArray(data?.rules) ? data.rules.length : 0
  );

  let uptime = $derived(() => {
    const started = data?.status?.started_at;
    if (!started) return '--';
    const elapsed = Math.floor((Date.now() - new Date(started).getTime()) / 1000);
    if (elapsed < 0) return '--';
    const d = Math.floor(elapsed / 86400);
    const h = Math.floor((elapsed % 86400) / 3600);
    const m = Math.floor((elapsed % 3600) / 60);
    if (d > 0) return `${d}d ${h}h`;
    if (h > 0) return `${h}h ${m}m`;
    return `${m}m`;
  });
</script>

<header class="topbar">
  <div class="topbar-left">
    <span class="status-dot" class:running={isRunning}></span>
    <span class="status-label">{isRunning ? 'Running' : 'Stopped'}</span>
  </div>

  <div class="topbar-center">
    <span class="chip">
      <svg class="chip-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M2 4h12M2 8h12M2 12h8" />
      </svg>
      {rulesCount} Rules
    </span>
    <span class="chip chip-tcp">
      <svg class="chip-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M2 4h4v8H2zM10 4h4v8h-4" />
        <path d="M6 7h4M6 9h4" />
      </svg>
      {totalTcp} TCP
    </span>
    <span class="chip chip-udp">
      <svg class="chip-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M3 12V6l5-3 5 3v6" />
        <path d="M3 6l5 3 5-3M8 9v6" />
      </svg>
      {totalUdp} UDP
    </span>
    <span class="chip chip-uptime">
      <svg class="chip-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="8" cy="8" r="6" />
        <path d="M8 4.5V8l2.5 1.5" />
      </svg>
      {uptime()}
    </span>
  </div>

  <div class="topbar-right">
    <button class="btn btn-accent btn-sm" onclick={onCreatePipeline}>
      + New Pipeline
    </button>
    <button class="btn btn-sm" onclick={onRefresh}>
      Refresh
    </button>
    <button class="btn btn-sm" onclick={onLogout}>
      Logout
    </button>
  </div>
</header>

<style>
  .topbar {
    height: var(--topbar-h);
    background: var(--bg-1);
    border-bottom: 1px solid var(--bg-4);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 20px;
    gap: 16px;
  }

  .topbar-left {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--err);
    flex-shrink: 0;
  }

  .status-dot.running {
    background: var(--ok);
    box-shadow: 0 0 6px rgba(112, 217, 138, 0.5);
  }

  .status-label {
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
  }

  .topbar-center {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    justify-content: center;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 500;
    color: var(--text-2);
    background: var(--bg-2);
    border: 1px solid var(--bg-4);
    white-space: nowrap;
  }

  .chip-icon {
    width: 13px;
    height: 13px;
    flex-shrink: 0;
  }

  .chip-tcp {
    color: var(--info);
    border-color: rgba(111, 175, 232, 0.25);
    background: rgba(111, 175, 232, 0.08);
  }

  .chip-udp {
    color: var(--accent);
    border-color: rgba(143, 227, 106, 0.25);
    background: rgba(143, 227, 106, 0.08);
  }

  .chip-uptime {
    color: var(--text-3);
  }

  .topbar-right {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }
</style>
