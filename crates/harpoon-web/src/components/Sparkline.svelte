<script>
  let { values = [], color = 'var(--accent)', width = 120, height = 32, label = '' } = $props();

  let path = $derived(() => {
    if (!values || values.length < 2) return '';
    const max = Math.max(...values, 1);
    const stepX = width / (values.length - 1);
    return values.map((v, i) => {
      const x = i * stepX;
      const y = height - (v / max) * (height - 4) - 2;
      return `${i === 0 ? 'M' : 'L'}${x.toFixed(1)},${y.toFixed(1)}`;
    }).join(' ');
  });

  let lastVal = $derived(values.length > 0 ? values[values.length - 1] : 0);
</script>

<div class="sparkline-wrap">
  {#if label}
    <span class="sparkline-label">{label}</span>
  {/if}
  <svg class="sparkline-svg" viewBox="0 0 {width} {height}" preserveAspectRatio="none">
    {#if path()}
      <path d={path()} fill="none" stroke={color} stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
    {/if}
  </svg>
  <span class="sparkline-value mono">{lastVal}</span>
</div>

<style>
  .sparkline-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .sparkline-label {
    font-size: 10px;
    color: var(--text-3);
    text-transform: uppercase;
    letter-spacing: .3px;
    min-width: 50px;
  }
  .sparkline-svg {
    width: 120px;
    height: 32px;
    flex-shrink: 0;
  }
  .sparkline-value {
    font-size: 12px;
    color: var(--text-2);
    min-width: 40px;
    text-align: right;
  }
</style>
