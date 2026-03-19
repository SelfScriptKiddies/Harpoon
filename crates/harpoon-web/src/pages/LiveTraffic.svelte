<script>
  import { onMount } from 'svelte';
  import { startCapture, stopCapture, fetchCapturePackets, fetchCaptureSessions, connectCaptureWs } from '../lib/api.js';

  let { data } = $props();

  function fmtTime(ms) { if(!ms) return '\u2014'; const d=new Date(ms); return d.toLocaleTimeString('en-GB',{hour12:false})+'.'+String(ms%1000).padStart(3,'0'); }

  function hexDump(hexStr, bytesPerLine = 16) {
    const lines = [];
    for (let i = 0; i < hexStr.length; i += bytesPerLine * 2) {
      const offset = (i / 2).toString(16).padStart(8, '0');
      const hexPart = hexStr.slice(i, i + bytesPerLine * 2).match(/.{1,2}/g)?.join(' ') || '';
      const asciiPart = hexStr.slice(i, i + bytesPerLine * 2).match(/.{1,2}/g)?.map(h => {
        const c = parseInt(h, 16);
        return c >= 32 && c < 127 ? String.fromCharCode(c) : '.';
      }).join('') || '';
      lines.push(`${offset}  ${hexPart.padEnd(bytesPerLine * 3 - 1)}  |${asciiPart}|`);
    }
    return lines.join('\n');
  }

  function hexToText(hexStr) {
    if (!hexStr) return '';
    const bytes = hexStr.match(/.{1,2}/g) || [];
    return bytes.map(h => {
      const c = parseInt(h, 16);
      return c >= 32 && c < 127 ? String.fromCharCode(c) : '.';
    }).join('');
  }

  function textPreview(pkt, maxLen = 80) {
    if (!pkt.payload_hex) return '';
    const text = hexToText(pkt.payload_hex);
    return text.length > maxLen ? text.slice(0, maxLen) + '\u2026' : text;
  }

  let rules = $derived(data?.rules ?? []);
  let rulesFull = $derived(data?.rulesFull ?? []);
  let stats = $derived(data?.stats ?? []);

  // State
  let ws = $state(null);
  let packets = $state([]);
  let selectedPacket = $state(null);
  let capturing = $state(false);
  let paused = $state(false);
  let selectedRule = $state('');
  let sessions = $state([]);
  let error = $state('');

  // Settings
  let maxPackets = $state(1000);
  let payloadSize = $state(4096);
  let timeout = $state(300);
  let showSettings = $state(false);

  // Inspector
  let inspectorView = $state('hex');

  // Derived
  let activeSessionCount = $derived(sessions.length);

  // Poll capture sessions every 2s
  let pollTimer = null;

  async function pollSessions() {
    try {
      sessions = await fetchCaptureSessions();
      // Update capturing state based on whether our selected rule has an active session
      if (selectedRule) {
        capturing = sessions.some(s => s.rule === selectedRule);
      }
    } catch {}
  }

  onMount(() => {
    // Connect WebSocket
    ws = connectCaptureWs(
      (pkt) => {
        if (!paused) {
          packets = [pkt, ...packets].slice(0, 2000);
        }
      },
      () => {
        ws = null;
      }
    );

    // Start session polling
    pollSessions();
    pollTimer = setInterval(pollSessions, 2000);

    // Set default selected rule
    if (rules.length > 0 && !selectedRule) {
      selectedRule = rules[0].name ?? rules[0];
    }

    return () => {
      if (ws) { ws.close(); ws = null; }
      if (pollTimer) clearInterval(pollTimer);
    };
  });

  async function handleStartCapture() {
    if (!selectedRule) return;
    error = '';
    try {
      await startCapture(selectedRule, { maxPackets, maxPayload: payloadSize, timeout });
      capturing = true;
    } catch (e) {
      error = 'Failed to start capture: ' + (e.message || e);
    }
  }

  async function handleStopCapture() {
    if (!selectedRule) return;
    error = '';
    try {
      await stopCapture(selectedRule);
      capturing = false;
    } catch (e) {
      error = 'Failed to stop capture: ' + (e.message || e);
    }
  }

  function togglePause() {
    paused = !paused;
  }

  function clearPackets() {
    packets = [];
    selectedPacket = null;
  }

  function selectPacket(pkt) {
    selectedPacket = pkt;
    inspectorView = 'hex';
  }

  async function copyPayload() {
    if (!selectedPacket?.payload_hex) return;
    const text = inspectorView === 'hex'
      ? hexDump(selectedPacket.payload_hex)
      : hexToText(selectedPacket.payload_hex);
    try {
      await navigator.clipboard.writeText(text);
    } catch {}
  }

  function getRuleName(r) {
    return typeof r === 'string' ? r : r.name;
  }
</script>

<div class="traffic-page" class:has-inspector={selectedPacket}>
  <!-- Page Header -->
  <div class="page-header">
    <h1 class="page-title">Live Traffic</h1>
    <span class="page-count mono">
      {packets.length} packets
      {#if activeSessionCount > 0}
        <span class="badge badge-ok">{activeSessionCount} active capture{activeSessionCount !== 1 ? 's' : ''}</span>
      {/if}
    </span>
  </div>

  <!-- Top Controls Bar -->
  <div class="controls-bar card">
    <div class="controls-row">
      <div class="control-group">
        <label>Rule</label>
        <select bind:value={selectedRule} style="width: 200px;">
          <option value="">-- select rule --</option>
          {#each rules as rule}
            <option value={getRuleName(rule)}>{getRuleName(rule)}</option>
          {/each}
        </select>
      </div>

      <div class="control-group">
        <label>&nbsp;</label>
        <div class="btn-group">
          <button class="btn btn-accent" onclick={handleStartCapture} disabled={!selectedRule || capturing}>
            Start Capture
          </button>
          <button class="btn btn-danger" onclick={handleStopCapture} disabled={!selectedRule || !capturing}>
            Stop Capture
          </button>
        </div>
      </div>

      <div class="control-group">
        <label>&nbsp;</label>
        <div class="btn-group">
          <button class="btn" class:btn-active={paused} onclick={togglePause}>
            {paused ? 'Resume' : 'Pause'}
          </button>
          <button class="btn" onclick={clearPackets}>Clear</button>
        </div>
      </div>

      <div class="control-group">
        <label>&nbsp;</label>
        <button class="btn btn-sm" onclick={() => showSettings = !showSettings}>
          {showSettings ? 'Hide Settings' : 'Settings'}
        </button>
      </div>

      {#if paused}
        <div class="pause-indicator">
          <span class="pause-dot"></span>
          Paused
        </div>
      {/if}

      {#if ws}
        <div class="ws-indicator">
          <span class="ws-dot connected"></span>
          <span class="mono" style="font-size:10px; color: var(--text-4);">WS</span>
        </div>
      {:else}
        <div class="ws-indicator">
          <span class="ws-dot"></span>
          <span class="mono" style="font-size:10px; color: var(--text-4);">WS</span>
        </div>
      {/if}
    </div>

    {#if showSettings}
      <div class="settings-row">
        <div class="control-group">
          <label>Max Packets</label>
          <input type="number" bind:value={maxPackets} min="1" max="100000" style="width: 100px;" />
        </div>
        <div class="control-group">
          <label>Payload Size</label>
          <input type="number" bind:value={payloadSize} min="64" max="65536" style="width: 100px;" />
        </div>
        <div class="control-group">
          <label>Timeout (s)</label>
          <input type="number" bind:value={timeout} min="1" max="3600" style="width: 100px;" />
        </div>
      </div>
    {/if}

    {#if error}
      <div class="error-bar">{error}</div>
    {/if}
  </div>

  <!-- Main Content -->
  <div class="main-content">
    <!-- Packet Stream -->
    <div class="packet-stream card">
      <div class="stream-header">
        <span class="section-title" style="margin-bottom:0;">Packet Stream</span>
        <span class="mono" style="font-size:11px; color:var(--text-4);">{packets.length} / 2000</span>
      </div>
      {#if packets.length === 0}
        <div class="empty-state">
          No packets captured yet. Select a rule and start capture.
        </div>
      {:else}
        <div class="table-wrap">
          <table class="packet-table">
            <thead>
              <tr>
                <th class="col-time">Time</th>
                <th class="col-dir">Dir</th>
                <th class="col-endpoints">Src / Dst</th>
                <th class="col-len">Len</th>
                <th class="col-preview">Preview</th>
              </tr>
            </thead>
            <tbody>
              {#each packets as pkt, i}
                <tr
                  class:selected={selectedPacket === pkt}
                  onclick={() => selectPacket(pkt)}
                >
                  <td class="mono col-time">{fmtTime(pkt.timestamp_ms)}</td>
                  <td class="col-dir">
                    {#if pkt.direction === 'c2s'}
                      <span class="badge badge-ok">c2s</span>
                    {:else if pkt.direction === 's2c'}
                      <span class="badge badge-tcp">s2c</span>
                    {:else}
                      <span class="badge">{pkt.direction ?? '?'}</span>
                    {/if}
                  </td>
                  <td class="mono col-endpoints">
                    {pkt.src ?? '\u2014'} <span class="arrow">\u2192</span> {pkt.dst ?? '\u2014'}
                  </td>
                  <td class="mono col-len">{pkt.payload_len ?? 0}</td>
                  <td class="col-preview mono">{textPreview(pkt)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>

    <!-- Packet Inspector -->
    {#if selectedPacket}
      <div class="packet-inspector card">
        <div class="inspector-header">
          <span class="section-title" style="margin-bottom:0;">Inspector</span>
          <button class="btn btn-sm" onclick={() => selectedPacket = null} style="margin-left:auto;">Close</button>
        </div>

        <!-- Metadata -->
        <div class="inspector-meta">
          <div class="meta-row">
            <span class="meta-label">Timestamp</span>
            <span class="meta-value mono">{fmtTime(selectedPacket.timestamp_ms)}</span>
          </div>
          <div class="meta-row">
            <span class="meta-label">Rule</span>
            <span class="meta-value mono">{selectedPacket.rule ?? '\u2014'}</span>
          </div>
          <div class="meta-row">
            <span class="meta-label">Direction</span>
            <span class="meta-value">
              {#if selectedPacket.direction === 'c2s'}
                <span class="badge badge-ok">c2s</span>
              {:else if selectedPacket.direction === 's2c'}
                <span class="badge badge-tcp">s2c</span>
              {:else}
                <span class="badge">{selectedPacket.direction ?? '?'}</span>
              {/if}
            </span>
          </div>
          <div class="meta-row">
            <span class="meta-label">Source</span>
            <span class="meta-value mono">{selectedPacket.src ?? '\u2014'}</span>
          </div>
          <div class="meta-row">
            <span class="meta-label">Destination</span>
            <span class="meta-value mono">{selectedPacket.dst ?? '\u2014'}</span>
          </div>
          <div class="meta-row">
            <span class="meta-label">Payload Length</span>
            <span class="meta-value mono">{selectedPacket.payload_len ?? 0} bytes</span>
          </div>
        </div>

        <!-- View Tabs -->
        <div class="inspector-tabs">
          <button
            class="tab-btn"
            class:tab-active={inspectorView === 'hex'}
            onclick={() => inspectorView = 'hex'}
          >Hex</button>
          <button
            class="tab-btn"
            class:tab-active={inspectorView === 'text'}
            onclick={() => inspectorView = 'text'}
          >Text</button>
          <button class="btn btn-sm" onclick={copyPayload} style="margin-left:auto;">Copy</button>
        </div>

        <!-- Payload View -->
        <div class="inspector-payload">
          {#if !selectedPacket.payload_hex}
            <div class="empty-state">No payload data.</div>
          {:else if inspectorView === 'hex'}
            <pre class="code-block">{hexDump(selectedPacket.payload_hex)}</pre>
          {:else}
            <pre class="code-block text-view">{hexToText(selectedPacket.payload_hex)}</pre>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .traffic-page { display: flex; flex-direction: column; gap: 16px; height: 100%; }

  .page-header { display: flex; align-items: baseline; gap: 12px; }
  .page-title { font-size: 22px; font-weight: 700; color: var(--text); }
  .page-count { font-size: 12px; color: var(--text-3); display: flex; align-items: center; gap: 8px; }

  /* Controls Bar */
  .controls-bar { display: flex; flex-direction: column; gap: 10px; }
  .controls-row { display: flex; align-items: flex-end; gap: 12px; flex-wrap: wrap; }
  .settings-row { display: flex; align-items: flex-end; gap: 12px; flex-wrap: wrap; padding-top: 8px; border-top: 1px solid var(--bg-4); }
  .control-group { display: flex; flex-direction: column; gap: 4px; }
  .control-group select { width: 200px; }

  .btn-group { display: flex; gap: 6px; }

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

  .ws-indicator {
    display: flex; align-items: center; gap: 4px; padding-bottom: 8px; margin-left: auto;
  }
  .ws-dot {
    width: 6px; height: 6px; border-radius: 50%; background: var(--err);
  }
  .ws-dot.connected {
    background: var(--ok); box-shadow: 0 0 4px rgba(112, 217, 138, 0.5);
  }

  .error-bar {
    padding: 6px 10px; background: rgba(226,109,109,.1); border: 1px solid var(--err);
    border-radius: var(--radius-sm); color: var(--err); font-size: 12px;
  }

  @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }

  /* Main Content — side-by-side when inspector open */
  .main-content { display: flex; gap: 16px; flex: 1; min-height: 0; }

  /* Packet Stream */
  .packet-stream { flex: 1; display: flex; flex-direction: column; overflow: hidden; min-width: 0; }
  .stream-header {
    display: flex; align-items: center; justify-content: space-between;
    margin-bottom: 8px;
  }
  .table-wrap {
    overflow: auto; flex: 1; max-height: calc(100vh - 320px);
  }

  .packet-table { width: 100%; border-collapse: collapse; font-size: 11px; }
  .packet-table thead th {
    text-align: left; padding: 6px 8px; font-size: 10px; font-weight: 600;
    color: var(--text-3); text-transform: uppercase; letter-spacing: .5px;
    border-bottom: 1px solid var(--bg-4); white-space: nowrap;
    position: sticky; top: 0; background: var(--bg-2); z-index: 1;
  }
  .packet-table tbody td {
    padding: 4px 8px; border-bottom: 1px solid var(--bg-3); color: var(--text-2);
    white-space: nowrap; cursor: pointer;
  }
  .packet-table tbody tr:hover { background: var(--bg-3); }
  .packet-table tbody tr.selected { background: rgba(143,227,106,.08); }
  .packet-table tbody tr.selected td { color: var(--text); }
  .packet-table tbody tr:last-child td { border-bottom: none; }

  .col-time { width: 110px; font-size: 10px; }
  .col-dir { width: 50px; }
  .col-endpoints { font-size: 10px; max-width: 280px; overflow: hidden; text-overflow: ellipsis; }
  .col-len { width: 60px; text-align: right; }
  .col-preview {
    font-size: 10px; color: var(--text-4); max-width: 300px;
    overflow: hidden; text-overflow: ellipsis;
  }
  .arrow { color: var(--text-4); margin: 0 2px; }

  /* Packet Inspector */
  .packet-inspector {
    width: 420px; min-width: 360px; display: flex; flex-direction: column;
    overflow: hidden; flex-shrink: 0;
  }
  .inspector-header {
    display: flex; align-items: center; gap: 8px; margin-bottom: 10px;
  }

  /* Metadata */
  .inspector-meta {
    display: flex; flex-direction: column; gap: 4px; margin-bottom: 10px;
    padding-bottom: 10px; border-bottom: 1px solid var(--bg-4);
  }
  .meta-row { display: flex; align-items: center; gap: 8px; font-size: 11px; }
  .meta-label {
    width: 90px; flex-shrink: 0; color: var(--text-4);
    text-transform: uppercase; font-size: 10px; letter-spacing: .3px;
  }
  .meta-value { color: var(--text-2); }

  /* Tabs */
  .inspector-tabs {
    display: flex; align-items: center; gap: 4px; margin-bottom: 8px;
  }
  .tab-btn {
    padding: 4px 12px; border-radius: var(--radius-sm); border: 1px solid var(--bg-4);
    background: var(--bg-1); color: var(--text-3); font-size: 11px;
    cursor: pointer; font-family: var(--font); transition: all .15s;
  }
  .tab-btn:hover { color: var(--text-2); background: var(--bg-3); }
  .tab-btn.tab-active {
    background: var(--bg-4); color: var(--text); border-color: var(--accent);
    font-weight: 600;
  }

  /* Payload */
  .inspector-payload { flex: 1; overflow: auto; min-height: 0; }
  .code-block {
    font-family: var(--mono); font-size: 11px; line-height: 1.6;
    background: var(--bg-0); border: 1px solid var(--bg-3); border-radius: var(--radius-sm);
    padding: 10px 12px; overflow: auto; color: var(--text-2);
    white-space: pre; tab-size: 8; max-height: calc(100vh - 520px);
  }
  .code-block.text-view {
    white-space: pre-wrap; word-break: break-all;
  }

  .empty-state { color: var(--text-4); font-size: 13px; padding: 20px 0; text-align: center; }
</style>
