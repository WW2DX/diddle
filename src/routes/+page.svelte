<script lang="ts">
  import { onMount } from "svelte";
  import Header from "$lib/Header.svelte";
  import Waterfall from "$lib/Waterfall.svelte";
  import DecoderView from "$lib/DecoderView.svelte";
  import EntryWindow from "$lib/EntryWindow.svelte";
  import FKeys from "$lib/FKeys.svelte";
  import Logbook from "$lib/Logbook.svelte";
  import CollapsiblePanel from "$lib/CollapsiblePanel.svelte";
  import SettingsPanel from "$lib/SettingsPanel.svelte";
  import BandmapPanel from "$lib/BandmapPanel.svelte";
  import type { RigState } from "$lib/tci";
  import { qsoLog } from "$lib/qsoLog.svelte";
  import { spots } from "$lib/spots.svelte";
  import { settings } from "$lib/settings.svelte";
  import { cluster } from "$lib/cluster.svelte";
  import { macroState } from "$lib/macros.svelte";

  // Rig state is owned here and propagated down. Header subscribes to the
  // backend tci:rig events and writes back via $bindable.
  let rig = $state<RigState>({ freq: 0, mode: "", ptt: false });

  onMount(() => {
    settings.load();
    macroState.load();
    qsoLog.load();
    spots.init();
    cluster.init();
  });
</script>

<Header bind:rig />

<main>
  <Waterfall />
  <DecoderView />
  <EntryWindow {rig} />
  <FKeys />
  <BandmapPanel {rig} />
  <Logbook />

  <CollapsiblePanel title="Settings — operator + contest" open={false}>
    <SettingsPanel />
  </CollapsiblePanel>
</main>

<style>
  :global(:root) {
    font-family:
      -apple-system,
      BlinkMacSystemFont,
      "Segoe UI",
      system-ui,
      sans-serif;
    color-scheme: dark;
    background: #0a0c0d;
    color: #e6e6e6;
    font-size: 14px;
  }

  :global(body) {
    margin: 0;
  }

  main {
    padding: 12px 16px 24px;
    max-width: 1400px;
    margin: 0 auto;
  }
</style>
