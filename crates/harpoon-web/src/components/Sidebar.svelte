<script>
  import { reload, stop } from '../lib/api.js';

  let { currentPage, onNavigate } = $props();

  const navItems = [
    { id: 'overview', label: 'Overview', icon: 'overview' },
    { id: 'rules', label: 'Rules', icon: 'rules' },
    { id: 'pipeline-editor', label: 'Pipeline', icon: 'pipeline' },
    { id: 'sessions', label: 'Sessions', icon: 'sessions' },
    { id: 'events', label: 'Events', icon: 'events' },
    { id: 'config', label: 'Config', icon: 'config' },
    { id: 'system', label: 'System', icon: 'system' },
  ];

  let reloading = $state(false);

  async function handleReload() {
    reloading = true;
    try {
      await reload();
    } catch { /* ignore */ }
    reloading = false;
  }

  async function handleShutdown() {
    if (!confirm('Are you sure you want to shut down Harpoon? This will stop all proxying.')) return;
    try {
      await stop();
    } catch { /* ignore */ }
  }
</script>

<aside class="sidebar">
  <div class="logo">HARPOON</div>

  <nav class="nav">
    {#each navItems as item}
      <div
        class="nav-item"
        class:active={currentPage === item.id}
        onclick={() => onNavigate(item.id)}
        role="button"
        tabindex="0"
        onkeydown={(e) => { if (e.key === 'Enter') onNavigate(item.id); }}
      >
        <svg class="nav-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          {#if item.icon === 'overview'}
            <rect x="2" y="2" width="5" height="5" rx="1" />
            <rect x="9" y="2" width="5" height="5" rx="1" />
            <rect x="2" y="9" width="5" height="5" rx="1" />
            <rect x="9" y="9" width="5" height="5" rx="1" />
          {:else if item.icon === 'rules'}
            <path d="M2 4h12M2 8h12M2 12h8" />
          {:else if item.icon === 'pipeline'}
            <circle cx="3" cy="8" r="1.5" />
            <circle cx="13" cy="4" r="1.5" />
            <circle cx="13" cy="12" r="1.5" />
            <path d="M4.5 8h3.5l2-4h1.5M8 8l2 4h1.5" />
          {:else if item.icon === 'sessions'}
            <path d="M2 4h4v8H2zM10 4h4v8h-4" />
            <path d="M6 7h4M6 9h4" />
          {:else if item.icon === 'events'}
            <path d="M9 2L4 9h4l-1 5 5-7H8l1-5z" />
          {:else if item.icon === 'config'}
            <circle cx="8" cy="8" r="2.5" />
            <path d="M8 2v2M8 12v2M2 8h2M12 8h2M3.8 3.8l1.4 1.4M10.8 10.8l1.4 1.4M3.8 12.2l1.4-1.4M10.8 5.2l1.4-1.4" />
          {:else if item.icon === 'system'}
            <rect x="2" y="3" width="12" height="8" rx="1.5" />
            <path d="M5 14h6M8 11v3" />
          {/if}
        </svg>
        <span class="nav-label">{item.label}</span>
      </div>
    {/each}
  </nav>

  <div class="sidebar-footer">
    <span class="version">harpoon v0.1.0</span>
    <div class="footer-actions">
      <button class="btn btn-sm" onclick={handleReload} disabled={reloading}>
        {reloading ? 'Reloading...' : 'Reload'}
      </button>
      <button class="btn btn-sm btn-danger" onclick={handleShutdown}>
        Shutdown
      </button>
    </div>
  </div>
</aside>

<style>
  .sidebar {
    grid-row: 1 / 3;
    width: var(--sidebar-w);
    background: var(--bg-1);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    border-right: 1px solid var(--bg-4);
  }

  .logo {
    font-family: var(--mono);
    font-size: 18px;
    font-weight: 700;
    color: var(--accent);
    letter-spacing: 3px;
    padding: 16px 20px;
    border-bottom: 1px solid var(--bg-4);
  }

  .nav {
    flex: 1;
    padding: 12px 0;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 20px;
    cursor: pointer;
    color: var(--text-3);
    font-size: 13px;
    transition: all 0.15s;
    border-left: 3px solid transparent;
    user-select: none;
  }

  .nav-item:hover {
    color: var(--text-2);
    background: var(--bg-2);
  }

  .nav-item.active {
    color: var(--accent);
    border-left-color: var(--accent);
    background: rgba(143, 227, 106, 0.06);
  }

  .nav-icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
  }

  .nav-label {
    white-space: nowrap;
  }

  .sidebar-footer {
    padding: 16px 20px;
    border-top: 1px solid var(--bg-4);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .version {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--text-4);
  }

  .footer-actions {
    display: flex;
    gap: 8px;
  }
</style>
