<script>
  let { onLogin } = $props();

  let username = $state('admin');
  let password = $state('');
  let error = $state('');
  let loading = $state(false);

  async function handleLogin() {
    if (loading) return;
    error = '';
    if (!username.trim() || !password) {
      error = 'Username and password are required.';
      return;
    }
    loading = true;
    try {
      await onLogin(username.trim(), password);
    } catch (e) {
      error = e?.message || 'Authentication failed.';
    }
    loading = false;
  }

  function handleKeydown(e) {
    if (e.key === 'Enter') handleLogin();
  }
</script>

<div class="login-backdrop">
  <div class="login-box">
    <h1 class="login-title">HARPOON</h1>
    <p class="login-subtitle">Network proxy administration</p>

    {#if error}
      <div class="login-error">{error}</div>
    {/if}

    <div class="field">
      <label for="login-user">Username</label>
      <input
        id="login-user"
        type="text"
        bind:value={username}
        disabled={loading}
        autocomplete="username"
      />
    </div>

    <div class="field">
      <label for="login-pass">Password</label>
      <input
        id="login-pass"
        type="password"
        bind:value={password}
        disabled={loading}
        autocomplete="current-password"
        autofocus
        onkeydown={handleKeydown}
      />
    </div>

    <button class="btn btn-accent login-btn" onclick={handleLogin} disabled={loading}>
      {loading ? 'Authenticating...' : 'Authenticate'}
    </button>
  </div>
</div>

<style>
  .login-backdrop {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    background: var(--bg-0);
  }

  .login-box {
    width: 360px;
    background: var(--bg-1);
    border: 1px solid var(--bg-4);
    border-radius: var(--radius);
    padding: 36px 32px 32px;
  }

  .login-title {
    font-family: var(--mono);
    font-size: 28px;
    font-weight: 700;
    color: var(--accent);
    letter-spacing: 4px;
    text-align: center;
    margin-bottom: 6px;
  }

  .login-subtitle {
    text-align: center;
    color: var(--text-3);
    font-size: 13px;
    margin-bottom: 28px;
  }

  .login-error {
    background: rgba(226, 109, 109, 0.12);
    border: 1px solid rgba(226, 109, 109, 0.3);
    color: var(--err);
    font-size: 12px;
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    margin-bottom: 16px;
  }

  .field {
    margin-bottom: 16px;
  }

  .login-btn {
    width: 100%;
    padding: 10px;
    font-size: 13px;
    margin-top: 8px;
  }

  .login-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
