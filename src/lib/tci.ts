// Thin TypeScript bindings over the Rust TCI client.
//
// All real work happens in Rust; this module just wraps invoke() / listen().

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type TciState =
  | { kind: "disconnected" }
  | { kind: "connecting" }
  | { kind: "connected"; url: string; ready: boolean }
  | { kind: "error"; message: string };

export interface RigState {
  freq: number;
  mode: string;
  ptt: boolean;
}

export const DEFAULT_TCI_URL = "ws://localhost:40001";

export async function connect(url: string): Promise<void> {
  await invoke("tci_connect", { url });
}

export async function disconnect(): Promise<void> {
  await invoke("tci_disconnect");
}

export async function status(): Promise<[TciState, RigState]> {
  return await invoke("tci_status");
}

export async function setFreq(hz: number): Promise<void> {
  await invoke("set_freq", { hz });
}

export async function setPtt(on: boolean): Promise<void> {
  await invoke("set_ptt", { on });
}

export async function transmit(text: string): Promise<void> {
  await invoke("transmit", { text });
}

/// Abort any in-flight transmission. No-op when nothing is TXing.
export async function txAbort(): Promise<void> {
  await invoke("tx_abort");
}

export async function sendRaw(raw: string): Promise<void> {
  await invoke("tci_send", { raw });
}

export async function audioStart(trx = 0): Promise<void> {
  await sendRaw(`audio_start:${trx}`);
}

export async function audioStop(trx = 0): Promise<void> {
  await sendRaw(`audio_stop:${trx}`);
}

export interface SpectrumFrame {
  mags_db: number[];
  fft_size: number;
  sample_rate: number;
  seq: number;
}

export function onSpectrum(
  cb: (f: SpectrumFrame) => void,
): Promise<UnlistenFn> {
  return listen<SpectrumFrame>("spectrum", (e) => cb(e.payload));
}

export function onRtty(cb: (chunk: string) => void): Promise<UnlistenFn> {
  return listen<string>("rtty", (e) => cb(e.payload));
}

export interface ScopeFrame {
  xs: number[];
  ys: number[];
}

export function onScope(cb: (f: ScopeFrame) => void): Promise<UnlistenFn> {
  return listen<ScopeFrame>("scope", (e) => cb(e.payload));
}

// ----- WAV file playback (for decoder testing) -----

export type WavStatus =
  | { kind: "idle" }
  | {
      kind: "playing";
      path: string;
      position_s: number;
      duration_s: number;
      sample_rate: number;
      channels: number;
    }
  | { kind: "done"; path: string }
  | { kind: "error"; message: string };

export async function playWav(path: string): Promise<void> {
  await invoke("play_wav", { path });
}

export async function stopWav(): Promise<void> {
  await invoke("stop_wav");
}

export async function wavStatus(): Promise<WavStatus> {
  return await invoke("wav_status");
}

export function onWavStatus(
  cb: (s: WavStatus) => void,
): Promise<UnlistenFn> {
  return listen<WavStatus>("wav:status", (e) => cb(e.payload));
}

// ----- Super Check Partial (callsign autocomplete) -----

export interface ScpStatus {
  count: number;
  source: string;
}

export async function scpSearch(query: string, max = 10): Promise<string[]> {
  return await invoke("scp_search", { query, max });
}

export async function scpLoadFile(path: string): Promise<ScpStatus> {
  return await invoke("scp_load_file", { path });
}

export async function scpStatus(): Promise<ScpStatus> {
  return await invoke("scp_status");
}

/// Returns the first SCP-matching callsign in `calls`, or "" if none match.
export async function scpContainsAny(calls: string[]): Promise<string> {
  return await invoke("scp_contains_any", { calls });
}

export interface ScpAutoDownloadResult {
  status: ScpStatus;
  path: string;
}

/// Download MASTER.SCP from supercheckpartial.com into the app data dir
/// and load it. Returns both the new status and the on-disk path so the
/// caller can persist it for future launches.
export async function scpAutoDownload(): Promise<ScpAutoDownloadResult> {
  return await invoke("scp_auto_download");
}

export interface BinaryFrameInfo {
  bytes: number;
  trx: number;
  sample_rate: number;
  format: number; // 0=i16 1=i24 2=i32 3=f32 4=f64
  codec: number;
  stream_type: number; // 0=iq 1=rx_audio 2=tx_audio 3=tx_chrono 4=spectrum
  channels: number;
  stream_label: string;
  fps: number;
}

export interface TciMsg {
  dir: "rx" | "tx";
  kind: "text" | "binary";
  text?: string;
  binary?: BinaryFrameInfo;
}

export function onState(cb: (s: TciState) => void): Promise<UnlistenFn> {
  return listen<TciState>("tci:state", (e) => cb(e.payload));
}

export function onRig(cb: (r: RigState) => void): Promise<UnlistenFn> {
  return listen<RigState>("tci:rig", (e) => cb(e.payload));
}

export function onMsg(cb: (m: TciMsg) => void): Promise<UnlistenFn> {
  return listen<TciMsg>("tci:msg", (e) => cb(e.payload));
}
