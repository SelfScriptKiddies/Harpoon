<script>
  import { getToken, clearToken, login, fetchStatus, fetchStats, fetchRules, fetchRulesFull, fetchEvents } from './lib/api.js';
  import Sidebar from './components/Sidebar.svelte';
  import Topbar from './components/Topbar.svelte';
  import Login from './pages/Login.svelte';
  import Overview from './pages/Overview.svelte';
  import Rules from './pages/Rules.svelte';
  import RuleEditor from './pages/RuleEditor.svelte';
  import PipelineEditor from './pages/PipelineEditor.svelte';
  import Sessions from './pages/Sessions.svelte';
  import Events from './pages/Events.svelte';
  import Config from './pages/Config.svelte';
  import System from './pages/System.svelte';

  let authenticated = $state(false);
  let currentPage = $state('overview');
  let data = $state({ status: null, stats: [], rules: [], rulesFull: [], events: [] });
  let editingRule = $state(null);       // for Simple mode (AppRule object or null)
  let editingPipeline = $state(null);   // for Pipeline mode (preset or null)

  async function checkAuth() {
    if (!getToken()) return;
    try {
      const status = await fetchStatus();
      if (status) { authenticated = true; await refreshAll(); }
    } catch { authenticated = false; }
  }

  async function handleLogin(user, pass) {
    await login(user, pass);
    authenticated = true;
    await refreshAll();
  }

  function handleLogout() {
    clearToken();
    authenticated = false;
  }

  async function refreshAll() {
    try {
      const [status, stats, rules, rulesFull, events] = await Promise.all([
        fetchStatus(), fetchStats(), fetchRules(), fetchRulesFull(), fetchEvents(),
      ]);
      data = { status, stats, rules, rulesFull, events };
    } catch { /* stale data ok */ }
  }

  function navigate(page) {
    editingRule = null;
    editingPipeline = null;
    currentPage = page;
  }

  // Open Simple form editor (from Rules page "Edit" button or "Create Rule")
  function openRuleEditor(rule) {
    editingRule = rule || null;
    editingPipeline = null;
    currentPage = 'rule-editor';
  }

  // Open Pipeline visual editor (from Rules page or from inside RuleEditor)
  function openPipelineEditor(preset) {
    editingPipeline = preset || null;
    editingRule = null;
    currentPage = 'pipeline-editor';
  }

  // Create new — defaults to Simple mode
  function createNew() {
    openRuleEditor(null);
  }

  checkAuth();
  setInterval(() => { if (authenticated) refreshAll(); }, 3000);
</script>

{#if !authenticated}
  <Login onLogin={handleLogin} />
{:else}
  <div class="app-layout">
    <Sidebar {currentPage} onNavigate={navigate} />
    <Topbar {data} onRefresh={refreshAll} onLogout={handleLogout} onCreatePipeline={createNew} />
    <main class="main-content">
      {#if currentPage === 'overview'}
        <Overview {data} onNavigate={navigate} onCreatePipeline={createNew} />
      {:else if currentPage === 'rules'}
        <Rules {data} onRefresh={refreshAll}
               onEditPipeline={openPipelineEditor}
               onEditRule={openRuleEditor}
               onCreateRule={createNew} />
      {:else if currentPage === 'rule-editor'}
        <RuleEditor rule={editingRule}
                    onSave={() => { navigate('rules'); refreshAll(); }}
                    onCancel={() => navigate('rules')} />
      {:else if currentPage === 'pipeline-editor'}
        <PipelineEditor preset={editingPipeline}
                        onSave={() => { navigate('rules'); refreshAll(); }}
                        onCancel={() => navigate('rules')} />
      {:else if currentPage === 'sessions'}
        <Sessions {data} />
      {:else if currentPage === 'events'}
        <Events {data} />
      {:else if currentPage === 'config'}
        <Config {data} onRefresh={refreshAll} />
      {:else if currentPage === 'system'}
        <System {data} />
      {/if}
    </main>
  </div>
{/if}

<style>
  .app-layout {
    display: grid;
    grid-template-columns: var(--sidebar-w) 1fr;
    grid-template-rows: var(--topbar-h) 1fr;
    height: 100vh;
    overflow: hidden;
  }
  .main-content {
    overflow-y: auto;
    padding: 24px;
    background: var(--bg-0);
  }
</style>
