// Quick WAV inspector — identifies the strongest spectral peaks in a recording
// so we can pick mark/space tones without guessing.
//
// Run:
//   cargo run --example analyze_wav -- /path/to/file.wav

use std::env;

use diddle_lib::dsp::{RttyConfig, RttyDemod};
use hound::WavReader;
use rustfft::{num_complex::Complex32, FftPlanner};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: analyze_wav <wav_file>");
        std::process::exit(1);
    }
    let path = &args[1];

    let mut reader = WavReader::open(path).expect("open wav");
    let spec = reader.spec();
    println!("== {} ==", path);
    println!(
        "  sample_rate={} channels={} bits={} fmt={:?}",
        spec.sample_rate, spec.channels, spec.bits_per_sample, spec.sample_format
    );

    let samples: Vec<f32> = match (spec.bits_per_sample, spec.sample_format) {
        (16, hound::SampleFormat::Int) => reader
            .samples::<i16>()
            .filter_map(Result::ok)
            .map(|s| s as f32 / 32_768.0)
            .collect(),
        (32, hound::SampleFormat::Float) => {
            reader.samples::<f32>().filter_map(Result::ok).collect()
        }
        _ => {
            eprintln!("unsupported format");
            std::process::exit(1);
        }
    };
    let dur_s = samples.len() as f64 / spec.sample_rate as f64;
    println!("  duration={:.2}s samples={}", dur_s, samples.len());

    // Fine-grained FFT for clean peak picking.
    let fft_size = 8192usize.min(next_pow2(samples.len()));
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(fft_size);

    // Hann window
    let window: Vec<f32> = (0..fft_size)
        .map(|i| {
            0.5 - 0.5
                * (2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32).cos()
        })
        .collect();

    // Average linear magnitude across all non-overlapping FFT frames.
    let mut avg = vec![0f64; fft_size / 2];
    let mut count = 0;
    for chunk in samples.chunks_exact(fft_size) {
        let mut buf: Vec<Complex32> = chunk
            .iter()
            .zip(window.iter())
            .map(|(s, w)| Complex32::new(s * w, 0.0))
            .collect();
        fft.process(&mut buf);
        for (i, c) in buf[..fft_size / 2].iter().enumerate() {
            avg[i] += (c.re * c.re + c.im * c.im).sqrt() as f64;
        }
        count += 1;
    }
    if count == 0 {
        eprintln!("no full FFT frames (file too short for fft_size={fft_size})");
        return;
    }
    let bin_hz = spec.sample_rate as f64 / fft_size as f64;
    let avg_db: Vec<f64> = avg
        .iter()
        .map(|&m| 20.0 * ((m / count as f64) + 1e-12).log10())
        .collect();

    println!(
        "  fft_size={} bin_hz={:.2} frames={}",
        fft_size, bin_hz, count
    );

    // Local-maxima peak finder above 100 Hz.
    let min_bin = (100.0 / bin_hz).ceil() as usize;
    let win = 8usize;
    let mut peaks: Vec<(usize, f64)> = Vec::new();
    for i in (min_bin + win)..(avg_db.len() - win) {
        let v = avg_db[i];
        if ((i - win)..=(i + win)).all(|j| j == i || avg_db[j] < v) {
            peaks.push((i, v));
        }
    }
    peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    println!("\n  top spectral peaks:");
    println!("  {:>10} {:>10}", "freq Hz", "level dB");
    for (bin, db) in peaks.iter().take(8) {
        println!("  {:>10.1} {:>10.1}", *bin as f64 * bin_hz, db);
    }

    // Guess the RTTY pair: two strongest peaks, sorted by frequency.
    if peaks.len() >= 2 {
        let mut top2 = peaks[..2].to_vec();
        top2.sort_by_key(|p| p.0);
        let f1 = top2[0].0 as f64 * bin_hz;
        let f2 = top2[1].0 as f64 * bin_hz;
        let shift = f2 - f1;
        println!(
            "\n  likely RTTY pair:  {:.1} Hz / {:.1} Hz   shift = {:.1} Hz",
            f1, f2, shift
        );
        let presets = [85.0, 170.0, 200.0, 425.0, 850.0];
        let closest = presets
            .iter()
            .min_by(|a, b| {
                (*a - shift)
                    .abs()
                    .partial_cmp(&(*b - shift).abs())
                    .unwrap()
            })
            .unwrap();
        println!(
            "  closest standard shift: {} Hz (delta {:.1} Hz)",
            closest,
            (closest - shift).abs()
        );
    }

    // Try decoding with multiple plausible settings.
    println!("\n  decoding trials:");
    let trials: Vec<(f32, f32, f32, &str)> = if peaks.len() >= 2 {
        // Use the strongest pair as one candidate.
        let mut top2 = peaks[..2].to_vec();
        top2.sort_by_key(|p| p.0);
        let f1 = top2[0].0 as f32 * bin_hz as f32;
        let f2 = top2[1].0 as f32 * bin_hz as f32;
        // Look for tighter pairs near 170 Hz shift among top 8 peaks.
        let top_n = peaks.iter().take(8).collect::<Vec<_>>();
        let mut close_170: Option<(f32, f32)> = None;
        for i in 0..top_n.len() {
            for j in 0..top_n.len() {
                if i == j {
                    continue;
                }
                let a = top_n[i].0 as f32 * bin_hz as f32;
                let b = top_n[j].0 as f32 * bin_hz as f32;
                let shift = b - a;
                if (shift - 170.0).abs() < 12.0 {
                    close_170 = Some((a, b));
                    break;
                }
            }
            if close_170.is_some() {
                break;
            }
        }
        let mut v = vec![
            (f1, f2, 45.45, "strongest pair @ 45.45 baud"),
            (f1, f2, 50.0, "strongest pair @ 50 baud"),
            (f1, f2, 75.0, "strongest pair @ 75 baud"),
        ];
        if let Some((a, b)) = close_170 {
            v.push((a, b, 45.45, "best 170-Hz pair @ 45.45 baud"));
            v.push((a, b, 50.0, "best 170-Hz pair @ 50 baud"));
        }
        v
    } else {
        vec![]
    };
    for (mark, space, baud, label) in trials {
        let text = try_decode(&samples, spec.sample_rate, mark, space, baud);
        let preview: String = text
            .chars()
            .take(120)
            .map(|c| match c {
                '\n' => ' ',
                c if (c as u32) < 0x20 => '?',
                c => c,
            })
            .collect();
        println!(
            "  [{:>5.1}/{:>5.1} @ {:>5.2}] {}: {:?}",
            mark, space, baud, label, preview
        );
    }

    // Quick baud-rate estimate: mark/space slicer → run-length histogram.
    if peaks.len() >= 2 {
        let mut top2 = peaks[..2].to_vec();
        top2.sort_by_key(|p| p.0);
        let mark_hz = top2[0].0 as f64 * bin_hz;
        let space_hz = top2[1].0 as f64 * bin_hz;
        let sr = spec.sample_rate as f64;
        // Goertzel-style narrow-band power per sample, smoothed.
        // For each tone, mix sample * exp(-j*2pi*f*t) and integrate over a
        // moving window of ~40 ms.
        let smooth_n = (sr * 0.04) as usize;
        let mut mark_pow = vec![0f64; samples.len()];
        let mut space_pow = vec![0f64; samples.len()];
        let mut mi = 0f64;
        let mut mq = 0f64;
        let mut si = 0f64;
        let mut sq = 0f64;
        let mut mi_buf = vec![0f64; smooth_n];
        let mut mq_buf = vec![0f64; smooth_n];
        let mut si_buf = vec![0f64; smooth_n];
        let mut sq_buf = vec![0f64; smooth_n];
        let mut bidx = 0;
        let two_pi = 2.0 * std::f64::consts::PI;
        for (n, &s) in samples.iter().enumerate() {
            let s = s as f64;
            let t = n as f64 / sr;
            let mci = (two_pi * mark_hz * t).cos();
            let mcq = (two_pi * mark_hz * t).sin();
            let sci = (two_pi * space_hz * t).cos();
            let scq = (two_pi * space_hz * t).sin();
            let nmi = s * mci;
            let nmq = s * mcq;
            let nsi = s * sci;
            let nsq = s * scq;
            mi += nmi - mi_buf[bidx];
            mq += nmq - mq_buf[bidx];
            si += nsi - si_buf[bidx];
            sq += nsq - sq_buf[bidx];
            mi_buf[bidx] = nmi;
            mq_buf[bidx] = nmq;
            si_buf[bidx] = nsi;
            sq_buf[bidx] = nsq;
            bidx = (bidx + 1) % smooth_n;
            mark_pow[n] = mi * mi + mq * mq;
            space_pow[n] = si * si + sq * sq;
        }
        // Slice
        let mut bits: Vec<bool> = mark_pow
            .iter()
            .zip(space_pow.iter())
            .map(|(m, s)| m > s)
            .collect();
        // Skip the settling region.
        let skip = smooth_n.min(bits.len());
        bits.drain(0..skip);
        // Run-length histogram
        let mut runs: Vec<usize> = Vec::new();
        let mut cur = bits[0];
        let mut len = 1usize;
        for &b in &bits[1..] {
            if b == cur {
                len += 1;
            } else {
                runs.push(len);
                cur = b;
                len = 1;
            }
        }
        runs.push(len);
        // Most common short run length ≈ samples per bit
        let mut hist = std::collections::HashMap::<usize, usize>::new();
        for r in &runs {
            // Round to nearest 4 samples to bucket
            let bucket = r / 4;
            *hist.entry(bucket).or_insert(0) += 1;
        }
        let mut hist_v: Vec<(usize, usize)> = hist.into_iter().collect();
        hist_v.sort_by(|a, b| b.1.cmp(&a.1));
        if let Some((bucket, hits)) = hist_v.first() {
            let samples_per_bit = (bucket * 4) as f64;
            let baud = sr / samples_per_bit;
            println!(
                "\n  baud guess: most common run-length = ~{:.0} samples → {:.2} baud  ({} runs in bucket)",
                samples_per_bit, baud, hits
            );
            let baud_presets = [45.45, 50.0, 75.0, 100.0];
            let closest_b = baud_presets
                .iter()
                .min_by(|a, b| {
                    (*a - baud)
                        .abs()
                        .partial_cmp(&(*b - baud).abs())
                        .unwrap()
                })
                .unwrap();
            println!(
                "  closest standard baud: {} (delta {:.2})",
                closest_b,
                (closest_b - baud).abs()
            );
        }
    }
}

fn next_pow2(n: usize) -> usize {
    let mut p = 1usize;
    while p < n {
        p <<= 1;
    }
    p
}

#[allow(dead_code)]
fn try_decode(samples: &[f32], sr: u32, mark_hz: f32, space_hz: f32, baud: f32) -> String {
    let mut d = RttyDemod::new(
        sr,
        RttyConfig {
            mark_hz,
            space_hz,
            baud,
        },
    );
    d.push(samples)
}
