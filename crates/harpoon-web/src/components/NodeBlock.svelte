<script>
  let { node, kindDef, selected = false, onMouseDown, onConnectStart, onConnectEnd, onRemove } = $props();
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="node-block"
     class:selected
     style="left: {node.x}px; top: {node.y}px; border-color: {selected ? kindDef?.color : 'var(--bg-4)'};"
     onmousedown={onMouseDown}
     onmouseup={onConnectEnd}>

  <div class="node-header" style="background: {kindDef?.color}20; border-bottom-color: {kindDef?.color}30;">
    <span class="node-dot" style="background: {kindDef?.color};"></span>
    <span class="node-label">{kindDef?.label || node.kind}</span>
    <button class="node-remove" onclick|stopPropagation={onRemove}>×</button>
  </div>

  <div class="node-body">
    {#if node.config?.listen}
      <div class="node-field mono">{node.config.listen}</div>
    {/if}
    {#if node.config?.target}
      <div class="node-field mono">{node.config.target}</div>
    {/if}
    {#if node.config?.pattern}
      <div class="node-field mono">{node.config.pattern}</div>
    {/if}
    {#if node.config?.protocol}
      <span class="node-badge">{node.config.protocol.toUpperCase()}</span>
    {/if}
  </div>

  <!-- Output port -->
  {#if kindDef?.ports?.out}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="node-port node-port-out" onmousedown|stopPropagation={onConnectStart}></div>
  {/if}
  <!-- Input port -->
  {#if kindDef?.ports?.in}
    <div class="node-port node-port-in"></div>
  {/if}
</div>

<style>
  .node-block {
    position: absolute;
    width: 220px;
    background: var(--bg-2);
    border: 1px solid var(--bg-4);
    border-radius: 10px;
    cursor: grab;
    user-select: none;
    transition: box-shadow .15s;
    z-index: 10;
  }
  .node-block:hover { box-shadow: 0 0 12px rgba(143,227,106,.1); }
  .node-block.selected { box-shadow: 0 0 16px rgba(143,227,106,.2); z-index: 20; }

  .node-header {
    display: flex; align-items: center; gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid;
    border-radius: 10px 10px 0 0;
    font-size: 12px; font-weight: 600;
  }
  .node-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .node-label { flex: 1; }
  .node-remove {
    background: none; border: none; color: var(--text-4); cursor: pointer;
    font-size: 16px; line-height: 1; padding: 0;
  }
  .node-remove:hover { color: var(--err); }

  .node-body { padding: 8px 12px; min-height: 24px; }
  .node-field { font-size: 11px; color: var(--text-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .node-badge {
    display: inline-block; padding: 1px 6px; border-radius: 999px;
    font-size: 9px; font-weight: 600; background: rgba(111,175,232,.15); color: var(--info);
  }

  .node-port {
    position: absolute; left: 50%; width: 12px; height: 12px;
    background: var(--bg-4); border: 2px solid var(--bg-1); border-radius: 50%;
    transform: translateX(-50%); cursor: crosshair; z-index: 30;
  }
  .node-port-out { bottom: -6px; }
  .node-port-in { top: -6px; }
  .node-port:hover { background: var(--accent); }
</style>
