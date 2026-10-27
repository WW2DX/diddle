# 6. Practice without a radio

The **Test** panel at the bottom of the page lets you learn Diddle, practise pileups or test the decoder without a radio, or without going on the air.

## Contest simulator

The simulator makes a synthetic RTTY band and feeds it through Diddle's real decoders. The header shows that the simulator is running, and **the radio is never keyed** while it runs.

**Pileup** mode is interactive. Press F1 (or Enter) to CQ and stations answer:

- Send a call and that station gives you his exchange. Send TU and the next one comes.
- Stations repeat on `AGN?` and correct a call you've miscopied.
- In a busy pileup, two can call at once.
- A station you ignore gives up and leaves.

**Playback** mode plays both sides of a scripted run back to back, like RTTY Runner. You listen and log; F-keys don't transmit.

Settings:

| Setting | What it changes |
|---------|-----------------|
| **Callers per CQ** | How many stations answer each CQ |
| **Callers off your tones** | How far callers spread from your frequency (practice using single-click RX tuning and AFC) |
| **Stations elsewhere in the passband** | Background stations that CQ, work each other and QSY, so tags, spots and the bandmap get exercised |
| **Noise, signal level** | HF-like noise and fading |
| **Pretend dial** | A frequency, so QSOs log on a real band |

Calls come from the SCP database, and exchanges follow the contest you've picked. A **truth panel** shows what each station really sent, so you can check your copy afterwards.

## Audio input

Decode any input device on your computer instead of the radio:

- A virtual audio cable (BlackHole on macOS, VB-CABLE on Windows) carrying another simulator's audio, such as [RTTY Runner](https://github.com/opalito/RTTYRunner).
- A sound card fed by a non-TCI radio. Receive only.

Click **↻** to list devices. On macOS, the first time, you'll be asked to allow microphone access.

## WAV file

**Load WAV…** plays a recording through the decoder. It's useful for comparing settings, or comparing DeepRTTY against the classic decoder, on the same audio.

Next: **[Keyboard reference →](07-keyboard.md)**
