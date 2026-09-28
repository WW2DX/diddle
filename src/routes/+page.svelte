<script lang="ts">
  import { onMount } from "svelte";
  import Header from "$lib/Header.svelte";
  import Waterfall from "$lib/Waterfall.svelte";
  import DecoderView from "$lib/DecoderView.svelte";
  import EntryWindow from "$lib/EntryWindow.svelte";
  import AdHocSend from "$lib/AdHocSend.svelte";
  import QuickActions from "$lib/QuickActions.svelte";
  import FKeys from "$lib/FKeys.svelte";
  import Logbook from "$lib/Logbook.svelte";
  import CollapsiblePanel from "$lib/CollapsiblePanel.svelte";
  import SettingsPanel from "$lib/SettingsPanel.svelte";
  import TestPanel from "$lib/TestPanel.svelte";
  import BandmapPanel from "$lib/BandmapPanel.svelte";
  import type { RigState } from "$lib/tci";
  import { qsoLog } from "$lib/qsoLog.svelte";
  import { spots } from "$lib/spots.svelte";
  import { settings } from "$lib/settings.svelte";
  import { cluster } from "$lib/cluster.svelte";
  import { macroState } from "$lib/macros.svelte";
  import { contestSetups } from "$lib/contestSetups.svelte";

  // Rig state is owned here and propagated down. Header subscribes to the
  // backend tci:rig events and writes back via $bindable.
  let rig = $state<RigState>({ freq: 0, mode: "", ptt: false });

  // UI zoom so the whole layout fits a laptop screen. Cmd/Ctrl with +/−/0
  // (and Cmd/Ctrl + scroll) scales the interface; the choice is persisted.
  const ZOOM_KEY = "diddle.zoom";
  let zoom = $state(1);

  function applyZoom() {
    (document.documentElement.style as any).zoom = String(zoom);
    // Viewport units aren't zoom-aware; the docked bandmap divides by this.
    document.documentElement.style.setProperty("--zoom", String(zoom));
  }

  // Dock the bandmap beside the operating panels only when there's room for
  // both: under ~1100 layout px the operating column would get cramped, so
  // it goes back below the F-keys, as before.
  const DOCK_MIN_WIDTH = 1100;
  let viewportW = $state(typeof window === "undefined" ? 0 : window.innerWidth);
  let docked = $derived(settings.bandmapSide && viewportW / zoom >= DOCK_MIN_WIDTH);

  function setZoom(z: number) {
    zoom = Math.min(2, Math.max(0.5, Math.round(z * 20) / 20));
    applyZoom();
    try {
      localStorage.setItem(ZOOM_KEY, String(zoom));
    } catch {}
  }

  function onZoomKey(e: KeyboardEvent) {
    if (!(e.metaKey || e.ctrlKey)) return;
    if (e.key === "=" || e.key === "+") {
      e.preventDefault();
      setZoom(zoom + 0.1);
    } else if (e.key === "-" || e.key === "_") {
      e.preventDefault();
      setZoom(zoom - 0.1);
    } else if (e.key === "0") {
      e.preventDefault();
      setZoom(1);
    }
  }

  function onZoomWheel(e: WheelEvent) {
    if (!(e.metaKey || e.ctrlKey)) return;
    e.preventDefault();
    setZoom(zoom + (e.deltaY < 0 ? 0.05 : -0.05));
  }

  // Keep the active saved contest setup in step with live edits so
  // re-loading it later brings back exactly what was used.
  $effect(() => {
    macroState.macros;
    settings.activeContest;
    settings.historyPath;
    contestSetups.syncActive();
  });

  onMount(() => {
    settings.load();
    macroState.load();
    contestSetups.load();
    qsoLog.load();
    spots.init();
    cluster.init();

    const stored = Number(localStorage.getItem(ZOOM_KEY));
    if (stored >= 0.5 && stored <= 2) zoom = stored;
    applyZoom();
    window.addEventListener("keydown", onZoomKey);
    window.addEventListener("wheel", onZoomWheel, { passive: false });
    const mainEl = document.querySelector("main");
    const onResize = () => {
      viewportW = window.innerWidth;
      if (mainEl) {
        document.documentElement.style.setProperty("--main-top", `${mainEl.offsetTop}px`);
      }
    };
    onResize();
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("resize", onResize);
      window.removeEventListener("keydown", onZoomKey);
      window.removeEventListener("wheel", onZoomWheel);
    };
  });
</script>

<Header bind:rig />

<main class:docked>
  <div class="ops">
    <Waterfall />
    <DecoderView {rig} />
    <EntryWindow {rig} />
    <FKeys />
    {#if !docked}
      <BandmapPanel {rig} />
    {/if}
    <Logbook />

    <CollapsiblePanel title="Settings — operator + contest" open={false}>
      <SettingsPanel />
    </CollapsiblePanel>

    <CollapsiblePanel title="Test — contest simulator, audio input, WAV" open={false}>
      <TestPanel />
    </CollapsiblePanel>
  </div>
  {#if docked}
    <aside class="bandmap-col">
      <BandmapPanel {rig} docked />
    </aside>
  {/if}
</main>

<AdHocSend />
<QuickActions />

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

  /* Bandmap docked beside the operating panels: it stays on screen while
     the page scrolls and fills the window height, so reading down a busy
     band never means scrolling away from the waterfall. */
  main.docked {
    max-width: 1880px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 440px;
    gap: 12px;
    align-items: start;
  }
  .ops { min-width: 0; }
  .bandmap-col {
    position: sticky;
    top: 12px;
    /* Leave room for the app header above <main>, so the column's bottom
       (the cluster command line) is on screen even before you scroll. */
    height: calc(100vh / var(--zoom, 1) - 24px - var(--main-top, 0px));
  }
</style>
