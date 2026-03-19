window.Pages = window.Pages || {};

Pages.nftables = {
  preview: null,

  render() {
    const el = document.getElementById('page-nftables');
    const rulesFull = App.data.rulesFull || [];
    const hasNftRules = rulesFull.some(r => r.name); // nft config is global, shown separately

    el.innerHTML = `
      <div class="section-title">nftables Integration</div>

      <div class="cards">
        <div class="card">
          <div class="card-label">Status</div>
          <div class="card-value" style="font-size:14px;" id="nft-status">Checking...</div>
        </div>
        <div class="card">
          <div class="card-label">Managed Table</div>
          <div class="card-value mono" style="font-size:14px;">ip harpoon</div>
        </div>
        <div class="card">
          <div class="card-label">Active Rules</div>
          <div class="card-value">${rulesFull.length}</div>
          <div class="card-sub">proxy rules (nft rules generated from config)</div>
        </div>
      </div>

      <div class="section">
        <div style="display:flex;align-items:center;gap:8px;margin-bottom:12px;">
          <div class="section-title" style="margin-bottom:0;">Generated Ruleset Preview</div>
          <button class="btn btn-sm" onclick="Pages.nftables.loadPreview()">Refresh</button>
          <button class="btn btn-sm" onclick="Pages.nftables.copyPreview()">Copy</button>
        </div>
        <div class="code-block" id="nft-preview" style="min-height:100px;">
          ${this.preview ? App.esc(this.preview) : '<span style="color:var(--text-4);">Click Refresh to load nft ruleset preview</span>'}
        </div>
      </div>

      <div class="section">
        <div class="section-title">Actions</div>
        <div style="display:flex;gap:8px;flex-wrap:wrap;">
          <button class="btn btn-accent" onclick="Pages.nftables.apply()">Apply Rules</button>
          <button class="btn btn-danger" onclick="Pages.nftables.rollback()">Rollback (Delete Table)</button>
        </div>
        <div style="margin-top:12px;padding:12px;background:var(--bg-3);border-radius:var(--radius-sm);border-left:3px solid var(--warn);">
          <div style="color:var(--warn);font-weight:600;font-size:12px;margin-bottom:4px;">⚠ Warning</div>
          <div style="color:var(--text-3);font-size:12px;">
            nftables operations modify kernel packet filtering rules. Apply only creates rules in the <code style="color:var(--accent);">ip harpoon</code> table.
            Rollback deletes this table entirely. Other firewall rules are not affected.
          </div>
        </div>
      </div>

      <div class="section">
        <div class="section-title">Supported Actions</div>
        <div class="tbl-wrap">
          <table>
            <thead><tr><th>Action</th><th>Description</th><th>Use Case</th></tr></thead>
            <tbody>
              <tr>
                <td><span class="badge badge-nft">REDIRECT</span></td>
                <td>Redirect traffic on a port to Harpoon listener</td>
                <td class="mono">tcp dport 80 redirect to :8080</td>
              </tr>
              <tr>
                <td><span class="badge badge-nft">DNAT</span></td>
                <td>Rewrite destination address</td>
                <td class="mono">udp dport 53 dnat to 10.0.0.5:5353</td>
              </tr>
              <tr>
                <td><span class="badge badge-nft">TPROXY</span></td>
                <td>Transparent proxy with original destination preservation</td>
                <td class="mono">tcp dport 443 tproxy to :8443</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    `;

    this.checkStatus();
  },

  async checkStatus() {
    try {
      const r = await App.api('/api/nft/status');
      const data = await r.json();
      const el = document.getElementById('nft-status');
      if (el) {
        if (data.available) {
          el.innerHTML = '<span class="badge badge-ok">AVAILABLE</span>';
        } else {
          el.innerHTML = '<span class="badge badge-err">NOT AVAILABLE</span>';
        }
      }
    } catch (e) {
      const el = document.getElementById('nft-status');
      if (el) el.innerHTML = '<span class="badge badge-muted">UNKNOWN</span>';
    }
  },

  async loadPreview() {
    try {
      const r = await App.api('/api/nft/preview');
      const data = await r.json();
      this.preview = data.ruleset || 'No rules to render';
      document.getElementById('nft-preview').textContent = this.preview;
    } catch (e) {
      App.toast('Failed to load preview', 'err');
    }
  },

  copyPreview() {
    if (this.preview) {
      navigator.clipboard.writeText(this.preview);
      App.toast('Copied to clipboard');
    }
  },

  async apply() {
    if (!confirm('Apply nftables rules? This will create/update the ip harpoon table.')) return;
    try {
      const r = await App.api('/api/nft/apply', { method: 'POST' });
      const data = await r.json();
      if (data.ok) App.toast('nft rules applied');
      else App.toast(data.error || 'Apply failed', 'err');
    } catch (e) {
      App.toast('Apply failed', 'err');
    }
  },

  async rollback() {
    if (!confirm('Delete the ip harpoon table? This removes all Harpoon nft rules.')) return;
    try {
      const r = await App.api('/api/nft/rollback', { method: 'POST' });
      const data = await r.json();
      if (data.ok) App.toast('nft table deleted');
      else App.toast(data.error || 'Rollback failed', 'err');
    } catch (e) {
      App.toast('Rollback failed', 'err');
    }
  }
};
