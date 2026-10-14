<script lang="ts">
  // Wraps an operating panel so it can be folded to a one-line bar. The
  // panel stays mounted (hidden) while folded, so the RX scrollback, the
  // waterfall history and the decoders keep running.
  import type { Snippet } from "svelte";
  import { layout, PANEL_TITLES, type PanelId } from "$lib/layout.svelte";

  // `fill`: the panel takes its container's full height (docked bandmap).
  let { id, fill = false, children }: { id: PanelId; fill?: boolean; children: Snippet } = $props();
  let folded = $derived(layout.folded[id]);
</script>

<div class="frame" class:fill>
  {#if folded}
    <button type="button" class="bar" onclick={() => layout.toggle(id)} title="Show {PANEL_TITLES[id]}">
      <span class="caret">▸</span>{PANEL_TITLES[id]}
    </button>
  {:else}
    <button type="button" class="fold" onclick={() => layout.toggle(id)} title="Fold {PANEL_TITLES[id]} away">▾</button>
  {/if}
  <div class="inner" class:hidden={folded}>
    {@render children()}
  </div>
</div>

<style>
  .frame { position: relative; }
  .frame.fill, .frame.fill .inner { height: 100%; }
  .hidden { display: none; }
  /* Sits in the panel's left padding, level with its title. */
  .fold {
    position: absolute;
    top: 11px;
    left: 2px;
    z-index: 2;
    background: none;
    border: none;
    padding: 0 2px;
    color: #6b7176;
    font-size: 11px;
    line-height: 1;
    cursor: pointer;
  }
  .fold:hover { color: #c5d1de; }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    margin-bottom: 8px;
    padding: 6px 12px;
    background: #181c1f;
    border: 1px solid #262b30;
    border-radius: 8px;
    color: #8a949d;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 1px;
    text-transform: uppercase;
    text-align: left;
    cursor: pointer;
  }
  .bar:hover { color: #e6e6e6; border-color: #3a4452; }
  .caret { color: #6b7176; }
</style>
