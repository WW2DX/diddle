<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { onMsg, sendRaw, type TciMsg } from "$lib/tci";

  let messages = $state<TciMsg[]>([]);
  let input = $state("");
  let logEl: HTMLDivElement | undefined;
  let autoScroll = $state(true);
  let filterBinary = $state(false);

  const MAX = 800;
  const unlisten: Array<() => void> = [];

  onMount(async () => {
    unlisten.push(
      await onMsg((m) => {
        // Mutate in place via spread to keep Svelte 5 reactivity simple.
        messages = [...messages, m];
        if (messages.length > MAX) {
          messages = messages.slice(-MAX);
        }
        if (autoScroll) {
          queueMicrotask(() => {
            if (logEl) logEl.scrollTop = logEl.scrollHeight;
          });
        }
      }),
    );
  });

  onDestroy(() => {
    for (const u of unlisten) u();
  });

  async function send() {
    const cmd = input.trim();
    if (!cmd) return;
    try {
      await sendRaw(cmd);
      input = "";
    } catch (e) {
      console.error("tci_send failed", e);
    }
  }

  async function quick(cmd: string) {
    try {
      await sendRaw(cmd);
    } catch (e) {
      console.error(e);
    }
  }

  function clear() {
    messages = [];
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter") send();
  }

  let visible = $derived(
    filterBinary ? messages.filter((m) => m.kind !== "binary") : messages,
  );

  // Common candidate commands to probe spectrum & audio streams.
  const probes = [
    "audio_start:0",
    "audio_stop:0",
    "iq_start:0",
    "iq_stop:0",
    "spectrum_enable:0,true",
    "spectrum_enable:0,false",
    "spectrum:0,true",
    "spectrum_start:0",
  ];
</script>

<section class="panel">
  <header>
    <h2>TCI console</h2>
    <div class="toolbar">
      <label>
        <input type="checkbox" bind:checked={autoScroll} />
        auto-scroll
      </label>
      <label>
        <input type="checkbox" bind:checked={filterBinary} />
        hide binary
      </label>
      <button class="small" onclick={clear}>clear</button>
    </div>
  </header>

  <div class="probes">
    <span class="probes-label">probe:</span>
    {#each probes as p}
      <button class="probe" onclick={() => quick(p)}>{p}</button>
    {/each}
  </div>

  <div class="log" bind:this={logEl}>
    {#each visible as m, i (i)}
      <div class="line dir-{m.dir} kind-{m.kind}">
        <span class="arrow">{m.dir === "tx" ? "▶" : "◀"}</span>
        {#if m.kind === "text"}
          <span class="text">{m.text}</span>
        {:else if m.binary}
          <span class="binary">
            ⟨{m.binary.stream_label}⟩
            <span class="meta">{m.binary.bytes}B · {m.binary.fps.toFixed(1)} fps</span>
          </span>
        {/if}
      </div>
    {/each}
  </div>

  <div class="input-row">
    <span class="prompt">tci&gt;</span>
    <input
      type="text"
      bind:value={input}
      onkeydown={onKey}
      placeholder="type a command, e.g. spectrum_enable:0,true"
      spellcheck="false"
    />
    <button onclick={send}>send</button>
  </div>
</section>

<style>
  .panel {
    background: #181c1f;
    border: 1px solid #262b30;
    border-radius: 8px;
    padding: 16px 18px;
    margin-bottom: 16px;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }

  h2 {
    margin: 0;
    font-size: 13px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: #8a949d;
    font-weight: 600;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 12px;
    color: #8a949d;
  }

  .toolbar label {
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
  }

  .probes {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    margin-bottom: 10px;
    font-size: 11px;
  }

  .probes-label {
    color: #8a949d;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-right: 4px;
  }

  .probe {
    background: #1f2630;
    border: 1px solid #2e3a4a;
    color: #c5d1de;
    border-radius: 3px;
    padding: 3px 7px;
    cursor: pointer;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
  }

  .probe:hover {
    background: #2a3445;
    border-color: #3a5a8a;
  }

  .log {
    background: #0c0e10;
    border: 1px solid #1f2429;
    border-radius: 4px;
    height: 280px;
    overflow-y: auto;
    padding: 6px 8px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    line-height: 1.45;
  }

  .line {
    display: flex;
    gap: 6px;
    padding: 0 2px;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .arrow {
    flex-shrink: 0;
    width: 12px;
  }

  .dir-tx .arrow {
    color: #4a90e2;
  }
  .dir-rx .arrow {
    color: #4ade80;
  }

  .dir-tx .text {
    color: #92c5fa;
  }
  .dir-rx .text {
    color: #c5d1de;
  }

  .kind-binary .binary {
    color: #fbbf24;
  }
  .meta {
    color: #6b7176;
    font-size: 10px;
    margin-left: 8px;
  }

  .input-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
  }

  .prompt {
    color: #4ade80;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
  }

  .input-row input {
    flex: 1;
    background: #0c0e10;
    border: 1px solid #2a2f33;
    border-radius: 3px;
    color: #c5d1de;
    padding: 5px 8px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
  }

  .input-row input:focus {
    outline: none;
    border-color: #4a90e2;
  }

  .input-row button,
  button.small {
    background: #2a3f5f;
    border: 1px solid #3a5a8a;
    color: #e6e6e6;
    border-radius: 3px;
    padding: 4px 10px;
    cursor: pointer;
    font-size: 12px;
  }

  button.small {
    padding: 2px 8px;
    font-size: 11px;
  }

  .input-row button:hover,
  button.small:hover {
    background: #345080;
  }
</style>
