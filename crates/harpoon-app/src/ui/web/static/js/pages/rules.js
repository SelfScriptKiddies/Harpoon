/*  Harpoon Web Admin Panel – Rules page
 *  ─────────────────────────────────────
 *  Full CRUD for proxy rules: list table, detail inspector,
 *  create / edit / clone / delete via modal form.
 */

window.Pages = window.Pages || {};

Pages.rules = (function () {
  'use strict';

  /* ── helpers ──────────────────────────────────────────────────── */

  const esc = (s) => App.esc(s == null ? '' : String(s));

  function fullRule(name) {
    return (App.data.rulesFull || []).find(r => r.name === name) || null;
  }

  function statsFor(name) {
    return (App.data.stats || []).find(s => s.rule_name === name) || {};
  }

  function protoBadge(proto) {
    const p = (proto || '').toLowerCase();
    return '<span class="badge badge-' + esc(p) + '">' + esc(p.toUpperCase()) + '</span>';
  }

  function actionBadge(action) {
    const a = (action || '').toLowerCase();
    if (a === 'drop')     return '<span class="badge badge-drop">' + esc(a) + '</span>';
    if (a === 'tap-only') return '<span class="badge badge-warn">' + esc(a) + '</span>';
    return '<span class="badge badge-ok">' + esc(a) + '</span>';
  }

  function kindBadge(kind) {
    const k = (kind || '').toLowerCase();
    return '<span class="badge badge-muted">' + esc(k) + '</span>';
  }

  function yesNo(val) {
    if (val) return '<span class="badge badge-ok">Yes</span>';
    return '<span class="badge badge-muted">No</span>';
  }

  function tlsBadge(full) {
    if (full && full.tls && full.tls.mode && full.tls.mode !== 'disabled') {
      return '<span class="badge badge-tls">' + esc(full.tls.mode) + '</span>';
    }
    return '<span class="badge badge-muted">-</span>';
  }

  /* currently selected rule name (for detail highlight) */
  let selectedRule = null;

  /* ── render() ────────────────────────────────────────────────── */

  function render() {
    const el = document.getElementById('page-rules');
    if (!el) return;

    const rules = App.data.rules || [];

    let html = '';

    /* header */
    html += '<div class="section">';
    html += '  <div class="section-title">';
    html += '    Rules <span class="count">' + rules.length + '</span>';
    html += '    <button class="btn btn-accent btn-sm" onclick="Pages.rules.openForm()" style="margin-left:auto">+ Create Rule</button>';
    html += '  </div>';

    if (rules.length === 0) {
      html += '  <div class="empty"><div class="empty-icon">&#x2696;</div>';
      html += '    <div class="empty-text">No rules configured yet. Create one to get started.</div>';
      html += '  </div>';
    } else {
      /* table */
      html += '  <div class="tbl-wrap">';
      html += '  <table>';
      html += '    <thead><tr>';
      html += '      <th>Name</th><th>Proto</th><th>Listen</th><th>Target</th>';
      html += '      <th>TLS</th><th>Filters</th><th>Dup</th><th>Exp</th><th>Actions</th>';
      html += '    </tr></thead>';
      html += '    <tbody>';

      for (const r of rules) {
        const full = fullRule(r.name);
        const sel = r.name === selectedRule ? ' style="background:var(--bg-3)"' : '';

        html += '<tr' + sel + '>';
        html += '  <td><a href="javascript:void(0)" onclick="Pages.rules.showDetail(\'' + esc(r.name) + '\')" class="mono">' + esc(r.name) + '</a></td>';
        html += '  <td>' + protoBadge(r.protocol) + '</td>';
        html += '  <td class="mono">' + esc(r.listen) + '</td>';
        html += '  <td class="mono">' + esc(r.target) + '</td>';
        html += '  <td>' + tlsBadge(full) + '</td>';
        html += '  <td>' + (r.filters_count || 0) + '</td>';
        html += '  <td>' + yesNo(r.has_duplicate) + '</td>';
        html += '  <td>' + yesNo(r.has_exporter) + '</td>';
        html += '  <td>';
        html += '    <button class="btn btn-sm" onclick="Pages.rules.editRule(\'' + esc(r.name) + '\')">Edit</button> ';
        html += '    <button class="btn btn-sm" onclick="Pages.rules.cloneRule(\'' + esc(r.name) + '\')">Clone</button> ';
        html += '    <button class="btn btn-sm btn-danger" onclick="Pages.rules.deleteRule(\'' + esc(r.name) + '\')">Delete</button>';
        html += '  </td>';
        html += '</tr>';
      }

      html += '    </tbody>';
      html += '  </table>';
      html += '  </div>';
    }

    html += '</div>';

    /* detail inspector placeholder */
    html += '<div id="rule-detail"></div>';

    el.innerHTML = html;

    /* re-render detail if a rule was selected */
    if (selectedRule) {
      renderDetail(selectedRule);
    }
  }

  /* ── showDetail / renderDetail ───────────────────────────────── */

  function showDetail(name) {
    selectedRule = name;
    render();
  }

  function renderDetail(name) {
    const container = document.getElementById('rule-detail');
    if (!container) return;

    const full = fullRule(name);
    const st = statsFor(name);

    if (!full) {
      container.innerHTML = '';
      return;
    }

    let html = '<div class="detail" style="margin-top:16px">';
    html += '<h3>' + esc(full.name) + '</h3>';

    /* basic info grid */
    html += detailRow('Protocol', protoBadge(full.protocol));
    html += detailRow('Listen', '<span class="mono">' + esc(full.listen) + '</span>');
    html += detailRow('Target', '<span class="mono">' + esc(full.target) + '</span>');

    if (full.tls && full.tls.mode && full.tls.mode !== 'disabled') {
      html += detailRow('TLS Mode', '<span class="badge badge-tls">' + esc(full.tls.mode) + '</span>');
      if (full.tls.ca_cert) html += detailRow('CA Cert', '<span class="mono">' + esc(full.tls.ca_cert) + '</span>');
      if (full.tls.ca_key)  html += detailRow('CA Key', '<span class="mono">' + esc(full.tls.ca_key) + '</span>');
    } else {
      html += detailRow('TLS Mode', '<span class="badge badge-muted">disabled</span>');
    }

    html += detailRow('Duplicate', full.duplicate
      ? '<span class="mono">' + esc(full.duplicate) + '</span>'
      : '<span class="badge badge-muted">none</span>');

    if (full.exporter) {
      let expVal = '<span class="badge badge-export">' + esc(full.exporter.kind) + '</span> ';
      if (full.exporter.kind === 'uds' && full.exporter.path) expVal += '<span class="mono">' + esc(full.exporter.path) + '</span>';
      if (full.exporter.kind === 'tcp' && full.exporter.addr) expVal += '<span class="mono">' + esc(full.exporter.addr) + '</span>';
      html += detailRow('Exporter', expVal);
    } else {
      html += detailRow('Exporter', '<span class="badge badge-muted">none</span>');
    }

    if (full.protocol === 'udp') {
      html += detailRow('UDP Source Mode', esc(full.udp_source_mode || 'proxy'));
      html += detailRow('Idle Timeout', (full.idle_timeout_secs != null ? full.idle_timeout_secs : 30) + 's');
    }

    /* filters */
    if (full.filters && full.filters.length > 0) {
      html += '<div style="margin-top:16px;margin-bottom:8px">';
      html += '<div class="section-title" style="font-size:14px">Filters <span class="count">' + full.filters.length + '</span></div>';
      for (const f of full.filters) {
        html += '<div class="filter-row" style="grid-template-columns:100px 1fr 80px 90px;margin-bottom:4px">';
        html += kindBadge(f.kind);
        html += '<span class="mono" style="overflow:hidden;text-overflow:ellipsis">' + esc(f.pattern) + '</span>';
        html += '<span>' + esc(f.direction || 'both') + '</span>';
        html += actionBadge(f.action);
        html += '</div>';
      }
      html += '</div>';
    }

    /* stats */
    html += '<div style="margin-top:20px">';
    html += '<div class="section-title" style="font-size:14px">Statistics</div>';
    html += '<div class="cards" style="grid-template-columns:repeat(auto-fill,minmax(160px,1fr))">';
    html += statCard('Bytes C2S', App.fmtBytes(st.bytes_client_to_server));
    html += statCard('Bytes S2C', App.fmtBytes(st.bytes_server_to_client));
    html += statCard('Packets C2S', fmtNum(st.packets_client_to_server));
    html += statCard('Packets S2C', fmtNum(st.packets_server_to_client));
    html += statCard('TCP Connections', fmtNum(st.active_tcp_connections));
    html += statCard('UDP Sessions', fmtNum(st.active_udp_sessions));
    html += statCard('Dropped Packets', fmtNum(st.dropped_packets), st.dropped_packets > 0 ? 'err' : '');
    html += statCard('Filter Matches', fmtNum(st.filter_matches));
    html += statCard('Export Drops', fmtNum(st.export_drops), st.export_drops > 0 ? 'warn' : '');
    html += '</div>';
    html += '</div>';

    html += '</div>';
    container.innerHTML = html;
  }

  function detailRow(key, valHtml) {
    return '<div class="detail-row"><span class="detail-key">' + esc(key) + '</span><span class="detail-val">' + valHtml + '</span></div>';
  }

  function statCard(label, value, cls) {
    return '<div class="card"><div class="card-label">' + esc(label) + '</div>'
         + '<div class="card-value' + (cls ? ' ' + cls : '') + '">' + esc(value) + '</div></div>';
  }

  function fmtNum(n) {
    if (n == null) return '0';
    return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  }

  /* ── openForm ────────────────────────────────────────────────── */

  function openForm(existingRule, isClone) {
    const isEdit = !!existingRule && !isClone;
    const r = existingRule || {};
    const title = isEdit ? 'Edit Rule' : (isClone ? 'Clone Rule' : 'Create Rule');
    const originalName = isEdit ? r.name : null;

    /* pre-fill defaults */
    const name         = r.name || '';
    const protocol     = r.protocol || 'tcp';
    const listen       = r.listen || '';
    const target       = r.target || '';
    const tlsMode      = (r.tls && r.tls.mode) || 'disabled';
    const caCert       = (r.tls && r.tls.ca_cert) || '';
    const caKey        = (r.tls && r.tls.ca_key) || '';
    const filters      = (r.filters && r.filters.length > 0) ? r.filters : [];
    const duplicate    = r.duplicate || '';
    const expKind      = (r.exporter && r.exporter.kind) || 'none';
    const expPath      = (r.exporter && r.exporter.path) || '';
    const expAddr      = (r.exporter && r.exporter.addr) || '';
    const idleTimeout  = r.idle_timeout_secs != null ? r.idle_timeout_secs : 30;
    const sourceMode   = r.udp_source_mode || 'proxy';

    const showTls = protocol === 'tcp';
    const showUdp = protocol === 'udp';
    const showCaCerts = tlsMode !== 'disabled';
    const showExpPath = expKind === 'uds';
    const showExpAddr = expKind === 'tcp';

    let html = '<h2>' + esc(title) + '</h2>';
    html += '<div id="rule-form-error" style="color:var(--err);font-size:13px;margin-bottom:12px;display:none"></div>';

    html += '<div style="display:grid;grid-template-columns:1fr 220px;gap:20px" id="rule-form-layout">';

    /* ── Left: form body ── */
    html += '<div id="rule-form-body">';

    /* Section 1: General */
    html += '<div class="form-section">';
    html += '  <div class="form-section-title">General</div>';
    html += '  <div>';
    html += '    <label for="rf-name">Rule Name</label>';
    html += '    <input type="text" id="rf-name" class="mono" value="' + esc(name) + '" placeholder="my-proxy-rule">';
    html += '    <label for="rf-protocol" style="margin-top:12px">Protocol</label>';
    html += '    <select id="rf-protocol" onchange="Pages.rules._onProtoChange()">';
    html += '      <option value="tcp"' + (protocol === 'tcp' ? ' selected' : '') + '>tcp</option>';
    html += '      <option value="udp"' + (protocol === 'udp' ? ' selected' : '') + '>udp</option>';
    html += '    </select>';
    html += '  </div>';
    html += '</div>';

    /* Section 2: Endpoints */
    html += '<div class="form-section">';
    html += '  <div class="form-section-title">Endpoints</div>';
    html += '  <div>';
    html += '    <label for="rf-listen">Listen Address</label>';
    html += '    <input type="text" id="rf-listen" class="mono" value="' + esc(listen) + '" placeholder="0.0.0.0:8080">';
    html += '    <label for="rf-target" style="margin-top:12px">Target Address</label>';
    html += '    <input type="text" id="rf-target" class="mono" value="' + esc(target) + '" placeholder="10.0.0.1:80">';
    html += '  </div>';
    html += '</div>';

    /* Section 3: TLS (collapsible, TCP only) */
    html += '<div class="collapsible' + (showCaCerts ? ' open' : '') + '" id="rf-tls-section" style="' + (showTls ? '' : 'display:none') + '">';
    html += '  <button type="button" class="collapsible-toggle" onclick="this.parentElement.classList.toggle(\'open\')">TLS Settings <span>&#9662;</span></button>';
    html += '  <div class="collapsible-content"><div style="padding:16px">';
    html += '    <label for="rf-tls-mode">TLS Mode</label>';
    html += '    <select id="rf-tls-mode" onchange="Pages.rules._onTlsModeChange()">';
    html += '      <option value="disabled"' + (tlsMode === 'disabled' ? ' selected' : '') + '>disabled</option>';
    html += '      <option value="passthrough"' + (tlsMode === 'passthrough' ? ' selected' : '') + '>passthrough</option>';
    html += '      <option value="terminate"' + (tlsMode === 'terminate' ? ' selected' : '') + '>terminate</option>';
    html += '      <option value="mitm"' + (tlsMode === 'mitm' ? ' selected' : '') + '>mitm</option>';
    html += '    </select>';
    html += '    <div id="rf-tls-certs" style="' + (showCaCerts ? '' : 'display:none') + '">';
    html += '      <label for="rf-ca-cert" style="margin-top:12px">CA Certificate Path</label>';
    html += '      <input type="text" id="rf-ca-cert" class="mono" value="' + esc(caCert) + '" placeholder="/path/to/ca.pem">';
    html += '      <label for="rf-ca-key" style="margin-top:12px">CA Key Path</label>';
    html += '      <input type="text" id="rf-ca-key" class="mono" value="' + esc(caKey) + '" placeholder="/path/to/ca-key.pem">';
    html += '    </div>';
    html += '  </div></div>';
    html += '</div>';

    /* Section 4: Filters (always visible) */
    html += '<div class="form-section">';
    html += '  <div class="form-section-title">Filters <button class="btn btn-sm" style="margin-left:auto" onclick="Pages.rules._addFilter()">+ Add Filter</button></div>';
    html += '  <div id="rf-filters">';
    if (filters.length === 0) {
      html += '    <div class="empty" style="padding:16px"><div class="empty-text">No filters. Click + Add Filter to add one.</div></div>';
    } else {
      for (let i = 0; i < filters.length; i++) {
        html += filterRowHtml(i, filters[i]);
      }
    }
    html += '  </div>';
    html += '</div>';

    /* Section 5: Duplication & Export (collapsible) */
    html += '<div class="collapsible' + ((duplicate || expKind !== 'none') ? ' open' : '') + '">';
    html += '  <button type="button" class="collapsible-toggle" onclick="this.parentElement.classList.toggle(\'open\')">Duplication &amp; Export <span>&#9662;</span></button>';
    html += '  <div class="collapsible-content"><div style="padding:16px">';
    html += '    <label for="rf-duplicate">Duplicate To</label>';
    html += '    <input type="text" id="rf-duplicate" class="mono" value="' + esc(duplicate) + '" placeholder="ip:port">';
    html += '    <label for="rf-exp-kind" style="margin-top:12px">Exporter Kind</label>';
    html += '    <select id="rf-exp-kind" onchange="Pages.rules._onExpKindChange()">';
    html += '      <option value="none"' + (expKind === 'none' ? ' selected' : '') + '>none</option>';
    html += '      <option value="uds"' + (expKind === 'uds' ? ' selected' : '') + '>uds</option>';
    html += '      <option value="tcp"' + (expKind === 'tcp' ? ' selected' : '') + '>tcp</option>';
    html += '    </select>';
    html += '    <div id="rf-exp-path-wrap" style="' + (showExpPath ? '' : 'display:none') + '">';
    html += '      <label for="rf-exp-path" style="margin-top:12px">Exporter Path (UDS)</label>';
    html += '      <input type="text" id="rf-exp-path" class="mono" value="' + esc(expPath) + '" placeholder="/tmp/harpoon.sock">';
    html += '    </div>';
    html += '    <div id="rf-exp-addr-wrap" style="' + (showExpAddr ? '' : 'display:none') + '">';
    html += '      <label for="rf-exp-addr" style="margin-top:12px">Exporter Address (TCP)</label>';
    html += '      <input type="text" id="rf-exp-addr" class="mono" value="' + esc(expAddr) + '" placeholder="127.0.0.1:9000">';
    html += '    </div>';
    html += '  </div></div>';
    html += '</div>';

    /* Section 6: UDP Settings (UDP only) */
    html += '<div id="rf-udp-section" style="' + (showUdp ? '' : 'display:none') + '">';
    html += '  <div class="form-section">';
    html += '    <div class="form-section-title">UDP Settings</div>';
    html += '    <div>';
    html += '      <label for="rf-idle-timeout">Idle Timeout (seconds)</label>';
    html += '      <input type="number" id="rf-idle-timeout" value="' + esc(String(idleTimeout)) + '" min="1" placeholder="30">';
    html += '      <label for="rf-source-mode" style="margin-top:12px">Source Mode</label>';
    html += '      <select id="rf-source-mode">';
    html += '        <option value="proxy"' + (sourceMode === 'proxy' ? ' selected' : '') + '>proxy</option>';
    html += '        <option value="preserve"' + (sourceMode === 'preserve' ? ' selected' : '') + '>preserve</option>';
    html += '      </select>';
    html += '    </div>';
    html += '  </div>';
    html += '</div>';

    html += '</div>'; /* end form body */

    /* ── Right: sticky summary ── */
    html += '<div>';
    html += '  <div class="sticky-summary" id="rf-summary">';
    html += '    <div style="font-size:12px;font-weight:600;color:var(--text-3);text-transform:uppercase;letter-spacing:0.05em;margin-bottom:12px">Summary</div>';
    html += '    <div id="rf-summary-content">';
    html += buildSummaryContent(protocol, listen, target, tlsMode, filters.length, duplicate, expKind);
    html += '    </div>';
    html += '  </div>';
    html += '</div>';

    html += '</div>'; /* end grid layout */

    /* Form actions */
    html += '<div style="display:flex;gap:8px;justify-content:flex-end;margin-top:20px;padding-top:16px;border-top:1px solid var(--bg-4)">';
    html += '  <button class="btn" onclick="App.closeModal()">Cancel</button>';
    html += '  <button class="btn" onclick="Pages.rules._validateForm()">Validate</button>';
    html += '  <button class="btn btn-accent" onclick="Pages.rules._saveRule(\'' + esc(originalName || '') + '\')">Save &amp; Apply</button>';
    html += '</div>';

    App.showModal(html);

    /* widen the modal for the two-column rule form + make responsive */
    const modal = document.querySelector('.modal');
    if (modal) {
      modal.style.maxWidth = '780px';
    }

    /* store filter counter for dynamic add/remove */
    _filterIdx = filters.length;

    /* update summary on any input change (debounced) */
    setTimeout(function () {
      _attachSummaryListeners();
      _applyResponsiveLayout();
    }, 0);
  }

  /* ── filter row HTML ─────────────────────────────────────────── */

  let _filterIdx = 0;

  function filterRowHtml(idx, f) {
    f = f || {};
    let html = '<div class="filter-row" id="rf-filter-' + idx + '">';
    html += '<select class="rf-flt-kind">';
    html += '  <option value="substr"' + ((f.kind || 'substr') === 'substr' ? ' selected' : '') + '>substr</option>';
    html += '  <option value="bsubstr"' + (f.kind === 'bsubstr' ? ' selected' : '') + '>bsubstr</option>';
    html += '  <option value="regex"' + (f.kind === 'regex' ? ' selected' : '') + '>regex</option>';
    html += '</select>';
    html += '<select class="rf-flt-dir">';
    html += '  <option value="both"' + ((f.direction || 'both') === 'both' ? ' selected' : '') + '>both</option>';
    html += '  <option value="c2s"' + (f.direction === 'c2s' ? ' selected' : '') + '>c2s</option>';
    html += '  <option value="s2c"' + (f.direction === 's2c' ? ' selected' : '') + '>s2c</option>';
    html += '</select>';
    html += '<input type="text" class="mono rf-flt-pattern" value="' + esc(f.pattern || '') + '" placeholder="pattern">';
    html += '<select class="rf-flt-action">';
    html += '  <option value="pass"' + ((f.action || 'pass') === 'pass' ? ' selected' : '') + '>pass</option>';
    html += '  <option value="drop"' + (f.action === 'drop' ? ' selected' : '') + '>drop</option>';
    html += '  <option value="tap-only"' + (f.action === 'tap-only' ? ' selected' : '') + '>tap-only</option>';
    html += '</select>';
    html += '<button class="btn btn-sm btn-danger" onclick="Pages.rules._removeFilter(' + idx + ')">&times;</button>';
    html += '</div>';
    return html;
  }

  /* ── form dynamic handlers ───────────────────────────────────── */

  function _onProtoChange() {
    const proto = document.getElementById('rf-protocol').value;
    const tlsSec = document.getElementById('rf-tls-section');
    const udpSec = document.getElementById('rf-udp-section');
    if (tlsSec) tlsSec.style.display = proto === 'tcp' ? '' : 'none';
    if (udpSec) udpSec.style.display = proto === 'udp' ? '' : 'none';
    _updateSummary();
  }

  function _onTlsModeChange() {
    const mode = document.getElementById('rf-tls-mode').value;
    const certs = document.getElementById('rf-tls-certs');
    if (certs) certs.style.display = mode !== 'disabled' ? '' : 'none';
    _updateSummary();
  }

  function _onExpKindChange() {
    const kind = document.getElementById('rf-exp-kind').value;
    const pathWrap = document.getElementById('rf-exp-path-wrap');
    const addrWrap = document.getElementById('rf-exp-addr-wrap');
    if (pathWrap) pathWrap.style.display = kind === 'uds' ? '' : 'none';
    if (addrWrap) addrWrap.style.display = kind === 'tcp' ? '' : 'none';
    _updateSummary();
  }

  function _addFilter() {
    const container = document.getElementById('rf-filters');
    if (!container) return;

    /* clear empty state if present */
    const emptyEl = container.querySelector('.empty');
    if (emptyEl) emptyEl.remove();

    const idx = _filterIdx++;
    const div = document.createElement('div');
    div.innerHTML = filterRowHtml(idx, {});
    container.appendChild(div.firstElementChild);
    _updateSummary();
  }

  function _removeFilter(idx) {
    const row = document.getElementById('rf-filter-' + idx);
    if (row) row.remove();

    /* show empty state if no filters remain */
    const container = document.getElementById('rf-filters');
    if (container && container.querySelectorAll('.filter-row').length === 0) {
      container.innerHTML = '<div class="empty" style="padding:16px"><div class="empty-text">No filters. Click + Add Filter to add one.</div></div>';
    }
    _updateSummary();
  }

  /* ── summary panel ───────────────────────────────────────────── */

  function buildSummaryContent(proto, listen, target, tlsMode, filterCount, dup, expKind) {
    let html = '';

    html += '<div style="margin-bottom:8px">' + protoBadge(proto) + '</div>';

    const listenStr = listen || '...';
    const targetStr = target || '...';
    html += '<div class="mono" style="font-size:12px;color:var(--text-2);margin-bottom:8px">' + esc(listenStr) + ' &rarr; ' + esc(targetStr) + '</div>';

    if (proto === 'tcp' && tlsMode && tlsMode !== 'disabled') {
      html += '<div style="margin-bottom:6px"><span class="badge badge-tls">' + esc(tlsMode) + '</span></div>';
    }

    html += '<div style="font-size:12px;color:var(--text-3);margin-bottom:4px">Filters: <span style="color:var(--text)">' + filterCount + '</span></div>';
    html += '<div style="font-size:12px;color:var(--text-3);margin-bottom:4px">Duplicate: <span style="color:var(--text)">' + (dup ? 'yes' : 'no') + '</span></div>';
    html += '<div style="font-size:12px;color:var(--text-3)">Exporter: <span style="color:var(--text)">' + (expKind && expKind !== 'none' ? 'yes' : 'no') + '</span></div>';

    return html;
  }

  function _updateSummary() {
    const el = document.getElementById('rf-summary-content');
    if (!el) return;

    const proto      = _val('rf-protocol');
    const listen     = _val('rf-listen');
    const target     = _val('rf-target');
    const tlsMode    = _val('rf-tls-mode');
    const filterCount = document.querySelectorAll('#rf-filters .filter-row').length;
    const dup        = _val('rf-duplicate');
    const expKind    = _val('rf-exp-kind');

    el.innerHTML = buildSummaryContent(proto, listen, target, tlsMode, filterCount, dup, expKind);
  }

  function _attachSummaryListeners() {
    const ids = ['rf-name', 'rf-protocol', 'rf-listen', 'rf-target', 'rf-tls-mode', 'rf-duplicate', 'rf-exp-kind'];
    for (const id of ids) {
      const el = document.getElementById(id);
      if (el) el.addEventListener('input', _updateSummary);
      if (el) el.addEventListener('change', _updateSummary);
    }
  }

  /* ── responsive layout ────────────────────────────────────────── */

  function _applyResponsiveLayout() {
    const layout = document.getElementById('rule-form-layout');
    if (!layout) return;
    if (layout.offsetWidth < 560) {
      layout.style.gridTemplateColumns = '1fr';
    } else {
      layout.style.gridTemplateColumns = '1fr 220px';
    }
  }

  /* ── collect form data ───────────────────────────────────────── */

  function _collectRule() {
    const proto = _val('rf-protocol');

    /* filters */
    const filterRows = document.querySelectorAll('#rf-filters .filter-row');
    const filters = [];
    for (const row of filterRows) {
      const kind    = row.querySelector('.rf-flt-kind').value;
      const dir     = row.querySelector('.rf-flt-dir').value;
      const pattern = row.querySelector('.rf-flt-pattern').value;
      const action  = row.querySelector('.rf-flt-action').value;
      filters.push({ kind: kind, direction: dir, pattern: pattern, action: action });
    }

    /* tls */
    const tlsMode = _val('rf-tls-mode');
    let tls = null;
    if (proto === 'tcp' && tlsMode && tlsMode !== 'disabled') {
      tls = {
        mode: tlsMode,
        ca_cert: _val('rf-ca-cert') || null,
        ca_key: _val('rf-ca-key') || null,
      };
    }

    /* exporter */
    const expKind = _val('rf-exp-kind');
    let exporter = null;
    if (expKind && expKind !== 'none') {
      exporter = {
        kind: expKind,
        path: expKind === 'uds' ? (_val('rf-exp-path') || null) : null,
        addr: expKind === 'tcp' ? (_val('rf-exp-addr') || null) : null,
      };
    }

    /* duplicate */
    const dupVal = _val('rf-duplicate');

    return {
      name:              _val('rf-name'),
      protocol:          proto,
      listen:            _val('rf-listen'),
      target:            _val('rf-target'),
      filters:           filters,
      duplicate:         dupVal || null,
      exporter:          exporter,
      tls:               tls,
      udp_source_mode:   proto === 'udp' ? _val('rf-source-mode') : null,
      idle_timeout_secs: proto === 'udp' ? parseInt(_val('rf-idle-timeout'), 10) || 30 : null,
    };
  }

  function _val(id) {
    const el = document.getElementById(id);
    return el ? el.value.trim() : '';
  }

  /* ── saveRule ─────────────────────────────────────────────────── */

  async function _saveRule(originalName) {
    const rule = _collectRule();
    _clearError();

    /* basic client-side validation */
    if (!rule.name) { _showError('Rule name is required.'); return; }
    if (!rule.listen) { _showError('Listen address is required.'); return; }
    if (!rule.target) { _showError('Target address is required.'); return; }

    try {
      let res;
      if (originalName) {
        res = await App.api('/api/rules/update', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ original_name: originalName, rule: rule }),
        });
      } else {
        res = await App.api('/api/rules/create', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(rule),
        });
      }

      if (res.ok) {
        App.toast(originalName ? 'Rule updated' : 'Rule created', 'ok');
        App.closeModal();
        await App.refreshAll();
      } else {
        const body = await res.text();
        _showError(body || 'Server returned ' + res.status);
      }
    } catch (e) {
      _showError('Request failed: ' + e.message);
    }
  }

  /* ── validateForm (dry-run) ──────────────────────────────────── */

  async function _validateForm() {
    const rule = _collectRule();
    _clearError();

    if (!rule.name) { _showError('Rule name is required.'); return; }
    if (!rule.listen) { _showError('Listen address is required.'); return; }
    if (!rule.target) { _showError('Target address is required.'); return; }

    try {
      const res = await App.api('/api/rules/create', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(Object.assign({}, rule, { dry_run: true })),
      });

      if (res.ok) {
        App.toast('Validation passed', 'ok');
      } else {
        const body = await res.text();
        _showError(body || 'Validation failed (status ' + res.status + ')');
      }
    } catch (e) {
      _showError('Validation request failed: ' + e.message);
    }
  }

  /* ── editRule ────────────────────────────────────────────────── */

  function editRule(name) {
    const r = fullRule(name);
    if (!r) {
      App.toast('Rule not found: ' + name, 'err');
      return;
    }
    openForm(r);
  }

  /* ── cloneRule ───────────────────────────────────────────────── */

  function cloneRule(name) {
    const r = fullRule(name);
    if (!r) {
      App.toast('Rule not found: ' + name, 'err');
      return;
    }
    const cloned = JSON.parse(JSON.stringify(r));
    cloned.name = r.name + '-copy';
    /* open as new (no originalName) */
    openForm(cloned, true);
  }

  /* ── deleteRule ──────────────────────────────────────────────── */

  async function deleteRule(name) {
    if (!confirm('Delete rule "' + name + '"? This cannot be undone.')) return;

    try {
      const res = await App.api('/api/rules/delete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: name }),
      });

      if (res.ok) {
        App.toast('Rule deleted: ' + name, 'ok');
        if (selectedRule === name) selectedRule = null;
        await App.refreshAll();
      } else {
        const body = await res.text();
        App.toast('Delete failed: ' + (body || res.status), 'err');
      }
    } catch (e) {
      App.toast('Delete failed: ' + e.message, 'err');
    }
  }

  /* ── error display in form ───────────────────────────────────── */

  function _showError(msg) {
    const el = document.getElementById('rule-form-error');
    if (el) {
      el.textContent = msg;
      el.style.display = '';
    }
  }

  function _clearError() {
    const el = document.getElementById('rule-form-error');
    if (el) {
      el.textContent = '';
      el.style.display = 'none';
    }
  }

  /* ── public API ──────────────────────────────────────────────── */

  return {
    render:           render,
    openForm:         openForm,
    showDetail:       showDetail,
    editRule:         editRule,
    cloneRule:        cloneRule,
    deleteRule:       deleteRule,

    /* internal handlers exposed for onclick bindings */
    _onProtoChange:   _onProtoChange,
    _onTlsModeChange: _onTlsModeChange,
    _onExpKindChange: _onExpKindChange,
    _addFilter:       _addFilter,
    _removeFilter:    _removeFilter,
    _saveRule:        _saveRule,
    _validateForm:    _validateForm,
  };
})();
