/** Harpoon API client */

let token = localStorage.getItem('harpoon_token') || '';

export function getToken() { return token; }
export function setToken(t) { token = t; localStorage.setItem('harpoon_token', t); }
export function clearToken() { token = ''; localStorage.removeItem('harpoon_token'); }

export async function api(path, opts = {}) {
  const headers = { 'Content-Type': 'application/json', ...(opts.headers || {}) };
  if (token) headers['Authorization'] = 'Bearer ' + token;
  const res = await fetch(path, { ...opts, headers });
  if (res.status === 401) {
    clearToken();
    throw new Error('unauthorized');
  }
  return res;
}

export async function login(username, password) {
  const res = await fetch('/api/auth/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password }),
  });
  if (!res.ok) throw new Error('Invalid credentials');
  const data = await res.json();
  setToken(data.token);
  return data.token;
}

export async function fetchStatus() { return (await api('/api/status')).json(); }
export async function fetchStats() { return (await api('/api/stats')).json(); }
export async function fetchRules() { return (await api('/api/rules')).json(); }
export async function fetchRulesFull() { return (await api('/api/rules/full')).json(); }
export async function fetchEvents() { return (await api('/api/events')).json(); }
export async function fetchConfigToml() { return (await api('/api/config/toml')).json(); }
export async function fetchNftStatus() { return (await api('/api/nft/status')).json(); }
export async function fetchNftPreview() { return (await api('/api/nft/preview')).json(); }

export async function createRule(rule) {
  const res = await api('/api/rules/create', { method: 'POST', body: JSON.stringify(rule) });
  return res.json();
}
export async function updateRule(originalName, rule) {
  const res = await api('/api/rules/update', { method: 'POST', body: JSON.stringify({ original_name: originalName, rule }) });
  return res.json();
}
export async function deleteRule(name) {
  const res = await api('/api/rules/delete', { method: 'POST', body: JSON.stringify({ name }) });
  return res.json();
}

export async function createPipeline(pipeline) {
  const res = await api('/api/pipelines/create', { method: 'POST', body: JSON.stringify(pipeline) });
  return res.json();
}
export async function updatePipeline(id, pipeline) {
  const res = await api('/api/pipelines/update', { method: 'POST', body: JSON.stringify({ id, pipeline }) });
  return res.json();
}
export async function validatePipeline(pipeline) {
  const res = await api('/api/pipelines/validate', { method: 'POST', body: JSON.stringify(pipeline) });
  return res.json();
}

export async function reload() { return (await api('/api/reload', { method: 'POST' })).json(); }
export async function stop() { return (await api('/api/stop', { method: 'POST' })).json(); }
export async function applyNft() { return (await api('/api/nft/apply', { method: 'POST' })).json(); }
export async function rollbackNft() { return (await api('/api/nft/rollback', { method: 'POST' })).json(); }
