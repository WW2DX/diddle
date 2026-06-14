<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    open = $bindable(true),
    children,
  }: { title: string; open?: boolean; children: Snippet } = $props();
</script>

<section class="panel" class:collapsed={!open}>
  <button class="head" onclick={() => (open = !open)}>
    <span class="caret">{open ? "▾" : "▸"}</span>
    <span class="title">{title}</span>
  </button>
  {#if open}
    <div class="body">
      {@render children()}
    </div>
  {/if}
</section>

<style>
  .panel {
    background: #181c1f;
    border: 1px solid #262b30;
    border-radius: 8px;
    margin-bottom: 12px;
    overflow: hidden;
  }

  .head {
    width: 100%;
    background: transparent;
    border: none;
    cursor: pointer;
    text-align: left;
    padding: 8px 16px;
    display: flex;
    align-items: center;
    gap: 8px;
    color: #8a949d;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 1px;
    font-weight: 600;
  }
  .head:hover {
    background: #1f2429;
    color: #c5d1de;
  }

  .caret {
    color: #5a636c;
    font-size: 10px;
    width: 12px;
    display: inline-block;
  }

  .body {
    padding: 0 16px 12px;
  }
</style>
