<script>
  /** Live stats overlay for the pipeline canvas.
   *  Shows packets/sec and bytes/sec on edges, counters on nodes.
   */
  import { fetchMetricsGlobal } from '../lib/api.js';

  let { nodes = [], edges = [], active = false } = $props();
  let metrics = $state(null);

  // Poll metrics when overlay is active
  let interval = $state(null);

  $effect(() => {
    if (active && !interval) {
      const poll = async () => {
        try {
          const data = await fetchMetricsGlobal();
          if (data?.points?.length > 0) {
            metrics = data.points[data.points.length - 1];
          }
        } catch {}
      };
      poll();
      interval = setInterval(poll, 1000);
    } else if (!active && interval) {
      clearInterval(interval);
      interval = null;
      metrics = null;
    }
  });

  function fmtRate(n) {
    if (!n) return '0';
    if (n < 1024) return n + '/s';
    if (n < 1048576) return (n / 1024).toFixed(0) + 'K/s';
    return (n / 1048576).toFixed(1) + 'M/s';
  }
</script>

{#if active && metrics}
  <!-- Edge labels -->
  {#each edges as edge}
    {@const fromNode = nodes.find(n => n.id === edge.from)}
    {@const toNode = nodes.find(n => n.id === edge.to)}
    {#if fromNode && toNode}
      {@const midX = (fromNode.x + toNode.x) / 2 + 110}
      {@const midY = (fromNode.y + 70 + toNode.y) / 2}
      <div class="overlay-edge-label" style="left:{midX}px;top:{midY}px;">
        <span class="overlay-rate">{fmtRate(metrics.bi + metrics.bo)}</span>
      </div>
    {/if}
  {/each}

  <!-- Node counters -->
  {#each nodes as node}
    {#if node.kind === 'forward'}
      <div class="overlay-node-counter" style="left:{node.x + 220}px;top:{node.y + 8}px;">
        <span class="overlay-tcp">{metrics.tcp || 0} tcp</span>
        <span class="overlay-udp">{metrics.udp || 0} udp</span>
      </div>
    {/if}
    {#if node.kind === 'filter'}
      <div class="overlay-node-counter" style="left:{node.x + 220}px;top:{node.y + 8}px;">
        {#if metrics.drops > 0}
          <span class="overlay-drops">{metrics.drops} drop</span>
        {/if}
      </div>
    {/if}
  {/each}
{/if}

<style>
  .overlay-edge-label {
    position: absolute;
    transform: translate(-50%, -50%);
    background: rgba(11, 15, 16, 0.85);
    border: 1px solid var(--bg-4);
    border-radius: 4px;
    padding: 2px 6px;
    pointer-events: none;
    z-index: 25;
  }
  .overlay-rate {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--accent);
  }
  .overlay-node-counter {
    position: absolute;
    display: flex;
    gap: 6px;
    pointer-events: none;
    z-index: 25;
  }
  .overlay-tcp, .overlay-udp, .overlay-drops {
    font-family: var(--mono);
    font-size: 9px;
    padding: 1px 4px;
    border-radius: 3px;
    background: rgba(11, 15, 16, 0.85);
  }
  .overlay-tcp { color: var(--info); }
  .overlay-udp { color: var(--accent); }
  .overlay-drops { color: var(--err); }
</style>
