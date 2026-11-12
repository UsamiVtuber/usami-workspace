// Escuta contínua do microfone no modo chamada. A thread é dona do `cpal::Stream`
// (que não atravessa a UI) e segmenta a fala por silêncio, devolvendo WAVs para
// transcrição — sem botão: quem fala recebe a vez.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

const MAX_UTTERANCE_SECONDS: usize = 30;
const READY_TIMEOUT: Duration = Duration::from_secs(3);
const STREAM_INIT_TIMEOUT: Duration = Duration::from_secs(2);

const FRAME_MS: usize = 20;
const PREROLL_MS: usize = 400;
const START_FRAMES: usize = 6; // 120 ms de voz abrem a fala
const SILENCE_FRAMES: usize = 45; // 900 ms de silêncio encerram o turno
const MIN_UTTERANCE_MS: usize = 350;
// Prévia ao vivo: a cada ~1,2 s de fala contínua uma cópia do que já foi dito
// vai para o STT, para a legenda do topo aparecer enquanto a pessoa fala.
const PARTIAL_MS: usize = 1200;
const PARTIAL_MIN_MS: usize = 600;

/// Portão único da escuta: a UI suspende enquanto pensa/fala e o usuário
/// suspende ao mutar. A thread de captura descarta tudo enquanto fechado.
pub struct MicGate {
    user: AtomicBool,
    agent: AtomicBool,
}

impl MicGate {
    pub fn new() -> Self {
        Self {
            // Começa fechado: só abre quando a escuta do modo chamada é ligada.
            user: AtomicBool::new(true),
            agent: AtomicBool::new(false),
        }
    }

    pub fn set_user(&self, suspend: bool) {
        self.user.store(suspend, Ordering::Relaxed);
    }

    pub fn set_agent(&self, suspend: bool) {
        self.agent.store(suspend, Ordering::Relaxed);
    }

    pub fn suspended(&self) -> bool {
        self.user.load(Ordering::Relaxed) || self.agent.load(Ordering::Relaxed)
    }
}

impl Default for MicGate {
    fn default() -> Self {
        Self::new()
    }
}

/// Fala entregue à UI. `turn` sobe a cada utterance: a prévia de um turno já
/// encerrado nunca deve reaparecer depois que o texto final foi aceito.
#[derive(Clone, Debug)]
pub struct VoiceSlice {
    pub turn: u64,
    pub wav: Vec<u8>,
}

pub struct VoiceListener {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    segments: Option<UnboundedReceiver<VoiceSlice>>,
    partials: Option<UnboundedReceiver<VoiceSlice>>,
}

impl VoiceListener {
    pub fn start(gate: Arc<MicGate>) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (segments_tx, segments_rx) = unbounded_channel();
        let (partials_tx, partials_rx) = unbounded_channel();
        let handle = thread::spawn(move || {
            capture_loop(thread_stop, gate, segments_tx, partials_tx, ready_tx)
        });

        match ready_rx.recv_timeout(READY_TIMEOUT) {
            Ok(Ok(())) => Ok(Self {
                stop,
                handle: Some(handle),
                segments: Some(segments_rx),
                partials: Some(partials_rx),
            }),
            Ok(Err(error)) => {
                let _ = handle.join();
                Err(error)
            }
            Err(_) => {
                stop.store(true, Ordering::Relaxed);
                let _ = handle.join();
                Err("O microfone não respondeu em 3 segundos.".into())
            }
        }
    }

    /// Entrega o canal de WAVs prontos. Só pode ser chamado uma vez.
    pub fn take_segments(&mut self) -> Option<UnboundedReceiver<VoiceSlice>> {
        self.segments.take()
    }

    /// Entrega o canal de prévias da fala em andamento (fatias parciais).
    pub fn take_partials(&mut self) -> Option<UnboundedReceiver<VoiceSlice>> {
        self.partials.take()
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for VoiceListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn capture_loop(
    stop: Arc<AtomicBool>,
    gate: Arc<MicGate>,
    segments: UnboundedSender<VoiceSlice>,
    partials: UnboundedSender<VoiceSlice>,
    ready: mpsc::Sender<Result<(), String>>,
) {
    if let Err(error) = capture_until_stopped(&stop, &gate, &segments, &partials, &ready) {
        // `start` fica esperando a inicialização; informar a falha evita 3 s
        // de espera até o timeout quando não há microfone ou formato válido.
        let _ = ready.send(Err(error));
    }
}

fn capture_until_stopped(
    stop: &AtomicBool,
    gate: &MicGate,
    segments: &UnboundedSender<VoiceSlice>,
    partials: &UnboundedSender<VoiceSlice>,
    ready: &mpsc::Sender<Result<(), String>>,
) -> Result<(), String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "Nenhum microfone de entrada encontrado.".to_string())?;
    let supported = device
        .default_input_config()
        .map_err(|error| format!("Falha ao consultar o microfone: {error}"))?;
    let sample_rate = supported.sample_rate();
    let channels = supported.channels() as usize;
    let frame_len = (sample_rate as usize * FRAME_MS / 1000).max(1);

    let queue = Arc::new(Mutex::new(VecDeque::<i16>::new()));
    let capacity = sample_rate as usize * 2; // 2 s de folga entre drenagens
    let stream = build_stream(
        &device,
        supported.sample_format(),
        supported.config(),
        Arc::clone(&queue),
        capacity,
        channels,
    )?;
    stream
        .play()
        .map_err(|error| format!("Falha ao iniciar a captura: {error}"))?;
    let _ = ready.send(Ok(()));

    let mut vad = Vad::new(sample_rate);
    let mut pending: Vec<i16> = Vec::with_capacity(frame_len * 4);
    while !stop.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(40));
        if let Ok(mut queued) = queue.lock() {
            pending.extend(queued.drain(..));
        }
        if gate.suspended() {
            // Enquanto a Usami fala (ou está muda), o microfone não transcreve
            // nem acumula: evita o loop de realimentação do próprio TTS.
            pending.clear();
            vad.reset();
            continue;
        }
        let complete = pending.len() - pending.len() % frame_len;
        for frame in pending[..complete].chunks_exact(frame_len) {
            if let Some(chunk) = vad.push_frame(frame) {
                let _ = segments.send(VoiceSlice {
                    turn: chunk.turn,
                    wav: encode_wav(&chunk.samples, sample_rate),
                });
            } else if let Some(chunk) = vad.partial_snapshot() {
                let _ = partials.send(VoiceSlice {
                    turn: chunk.turn,
                    wav: encode_wav(&chunk.samples, sample_rate),
                });
            }
        }
        pending.drain(..complete);
    }
    drop(stream);
    Ok(())
}

fn build_stream(
    device: &cpal::Device,
    format: cpal::SampleFormat,
    config: cpal::StreamConfig,
    queue: Arc<Mutex<VecDeque<i16>>>,
    capacity: usize,
    channels: usize,
) -> Result<cpal::Stream, String> {
    macro_rules! build {
        ($sample:ty, $convert:expr) => {{
            let queue = Arc::clone(&queue);
            let convert = $convert;
            device.build_input_stream::<$sample, _, _>(
                config,
                move |data: &[$sample], _| {
                    push_queued(&queue, data, channels, capacity, &convert);
                },
                report_stream_error,
                Some(STREAM_INIT_TIMEOUT),
            )
        }};
    }

    let stream = match format {
        cpal::SampleFormat::F32 => build!(f32, f32_to_i16),
        cpal::SampleFormat::I16 => build!(i16, |sample| sample),
        cpal::SampleFormat::U16 => build!(u16, u16_to_i16),
        cpal::SampleFormat::I32 => build!(i32, |sample| (sample >> 16) as i16),
        cpal::SampleFormat::F64 => build!(f64, f64_to_i16),
        other => return Err(format!("Formato de microfone não suportado: {other:?}.")),
    };
    stream.map_err(|error| format!("Falha ao abrir o microfone: {error}"))
}

fn report_stream_error(error: cpal::Error) {
    eprintln!("[Usami Mic] {error}");
}

fn f32_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

fn f64_to_i16(sample: f64) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f64) as i16
}

fn u16_to_i16(sample: u16) -> i16 {
    (sample as i32 - 32768) as i16
}

fn push_queued<T, F>(
    queue: &Arc<Mutex<VecDeque<i16>>>,
    data: &[T],
    channels: usize,
    capacity: usize,
    convert: &F,
) where
    T: Copy,
    F: Fn(T) -> i16,
{
    let Ok(mut queued) = queue.lock() else {
        return;
    };
    let channels = channels.max(1);
    for sample in data.iter().step_by(channels) {
        if queued.len() >= capacity {
            queued.pop_front();
        }
        queued.push_back(convert(*sample));
    }
}

fn frame_rms(frame: &[i16]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum: f64 = frame
        .iter()
        .map(|sample| {
            let value = *sample as f64 / 32_768.0;
            value * value
        })
        .sum();
    (sum / frame.len() as f64).sqrt() as f32
}

enum VadState {
    Idle {
        loud_frames: usize,
    },
    Speech {
        samples: Vec<i16>,
        silence_frames: usize,
        /// Quadros desde a última prévia enviada ao STT.
        partial_frames: usize,
    },
}

/// Amostras de um turno de fala (final ou prévia).
struct VadChunk {
    turn: u64,
    samples: Vec<i16>,
}

/// Detector de atividade de voz por energia, com piso de ruído adaptativo e
/// pré-roll para não cortar o início da frase. Recebe quadros de 20 ms e devolve
/// uma fala completa quando o silêncio passa de ~900 ms.
struct Vad {
    preroll_cap: usize,
    start_frames: usize,
    silence_frames: usize,
    min_samples: usize,
    max_samples: usize,
    partial_every: usize,
    partial_min_samples: usize,
    noise_floor: f32,
    preroll: VecDeque<i16>,
    state: VadState,
    turn: u64,
}

impl Vad {
    fn new(sample_rate: u32) -> Self {
        let samples = |ms: usize| (sample_rate as usize * ms / 1000).max(1);
        Self {
            preroll_cap: samples(PREROLL_MS),
            start_frames: START_FRAMES,
            silence_frames: SILENCE_FRAMES,
            min_samples: samples(MIN_UTTERANCE_MS),
            max_samples: sample_rate as usize * MAX_UTTERANCE_SECONDS,
            partial_every: PARTIAL_MS / FRAME_MS,
            partial_min_samples: samples(PARTIAL_MIN_MS),
            noise_floor: 0.0,
            preroll: VecDeque::new(),
            state: VadState::Idle { loud_frames: 0 },
            turn: 0,
        }
    }

    fn push_frame(&mut self, frame: &[i16]) -> Option<VadChunk> {
        let rms = frame_rms(frame);
        match std::mem::replace(&mut self.state, VadState::Idle { loud_frames: 0 }) {
            VadState::Idle { mut loud_frames } => {
                if rms > self.start_threshold() {
                    loud_frames += 1;
                } else {
                    loud_frames = 0;
                    self.noise_floor = self.noise_floor * 0.95 + rms * 0.05;
                }
                self.push_preroll(frame);
                if loud_frames >= self.start_frames {
                    let samples = self.preroll.drain(..).collect();
                    self.turn += 1;
                    self.state = VadState::Speech {
                        samples,
                        silence_frames: 0,
                        // A prévia conta só a fala deste turno, não o pré-roll.
                        partial_frames: 0,
                    };
                } else {
                    self.state = VadState::Idle { loud_frames };
                }
                None
            }
            VadState::Speech {
                mut samples,
                mut silence_frames,
                mut partial_frames,
            } => {
                samples.extend_from_slice(frame);
                partial_frames += 1;
                if rms > self.continue_threshold() {
                    silence_frames = 0;
                } else {
                    silence_frames += 1;
                }
                if silence_frames >= self.silence_frames || samples.len() >= self.max_samples {
                    let turn = self.turn;
                    if samples.len() >= self.min_samples {
                        Some(VadChunk { turn, samples })
                    } else {
                        None
                    }
                } else {
                    self.state = VadState::Speech {
                        samples,
                        silence_frames,
                        partial_frames,
                    };
                    None
                }
            }
        }
    }

    /// Enquanto a fala continua, devolve a cada ~1,2 s uma cópia do que já foi
    /// dito para o STT montar a legenda ao vivo. O turno final não depende dela.
    fn partial_snapshot(&mut self) -> Option<VadChunk> {
        let VadState::Speech {
            samples,
            partial_frames,
            ..
        } = &mut self.state
        else {
            return None;
        };
        if *partial_frames < self.partial_every || samples.len() < self.partial_min_samples {
            return None;
        }
        *partial_frames = 0;
        Some(VadChunk {
            turn: self.turn,
            samples: samples.clone(),
        })
    }

    fn reset(&mut self) {
        self.preroll.clear();
        self.state = VadState::Idle { loud_frames: 0 };
    }

    fn push_preroll(&mut self, frame: &[i16]) {
        self.preroll.extend(frame.iter().copied());
        while self.preroll.len() > self.preroll_cap {
            self.preroll.pop_front();
        }
    }

    fn start_threshold(&self) -> f32 {
        (self.noise_floor * 2.5).clamp(0.008, 0.08)
    }

    fn continue_threshold(&self) -> f32 {
        (self.noise_floor * 1.5).clamp(0.005, 0.06)
    }
}

fn encode_wav(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 16_000;
    const FRAME: usize = RATE as usize * FRAME_MS / 1000;

    fn speech_frame() -> Vec<i16> {
        vec![3_000; FRAME]
    }

    fn silence_frame() -> Vec<i16> {
        vec![0; FRAME]
    }

    fn feed(vad: &mut Vad, frame: &[i16], times: usize) -> Option<Vec<i16>> {
        let mut out = None;
        for _ in 0..times {
            if let Some(chunk) = vad.push_frame(frame) {
                out = Some(chunk.samples);
            }
        }
        out
    }

    #[test]
    fn vad_emits_utterance_after_speech_and_silence() {
        let mut vad = Vad::new(RATE);
        assert!(feed(&mut vad, &silence_frame(), 10).is_none());
        assert!(feed(&mut vad, &speech_frame(), 20).is_none());

        let utterance =
            feed(&mut vad, &silence_frame(), SILENCE_FRAMES + 1).expect("silêncio encerra o turno");
        assert!(utterance.len() >= vad.min_samples);
        // pré-roll (10 quadros de silêncio) + 20 de fala + 45 de silêncio final
        assert!(utterance.len() <= (10 + 20 + SILENCE_FRAMES + 1) * FRAME);
        assert!(utterance.iter().any(|sample| *sample != 0));
    }

    #[test]
    fn vad_ignores_short_blips() {
        let mut vad = Vad::new(RATE);
        // Dois quadros barulhentos não bastam para abrir a fala (mínimo: 6).
        assert!(feed(&mut vad, &speech_frame(), 2).is_none());
        assert!(feed(&mut vad, &silence_frame(), SILENCE_FRAMES + 5).is_none());
    }

    #[test]
    fn vad_emits_partial_snapshots_while_speaking() {
        let mut vad = Vad::new(RATE);
        assert!(feed(&mut vad, &speech_frame(), 10).is_none());
        // Antes de 1,2 s de fala contínua ainda não há prévia.
        assert!(vad.partial_snapshot().is_none());

        let frames = PARTIAL_MS / FRAME_MS;
        for _ in 0..frames {
            assert!(vad.push_frame(&speech_frame()).is_none());
        }
        let partial = vad.partial_snapshot().expect("prévia a cada 1,2 s");
        assert_eq!(partial.turn, 1);
        assert!(partial.samples.len() >= frames * FRAME);
        assert!(partial.samples.iter().all(|sample| *sample != 0));

        // Logo depois de enviar, o contador recomeça.
        assert!(vad.partial_snapshot().is_none());

        // O turno final carrega o mesmo id da prévia.
        let mut final_chunk = None;
        for _ in 0..=SILENCE_FRAMES {
            if let Some(chunk) = vad.push_frame(&silence_frame()) {
                final_chunk = Some(chunk);
            }
        }
        assert_eq!(final_chunk.expect("silêncio encerra o turno").turn, 1);
    }

    #[test]
    fn vad_reset_discards_partial_utterance() {
        let mut vad = Vad::new(RATE);
        assert!(feed(&mut vad, &speech_frame(), 20).is_none());
        vad.reset();
        assert!(feed(&mut vad, &silence_frame(), SILENCE_FRAMES + 1).is_none());
        // Depois do reset uma nova fala ainda é reconhecida.
        assert!(feed(&mut vad, &speech_frame(), 20).is_none());
        assert!(feed(&mut vad, &silence_frame(), SILENCE_FRAMES + 1).is_some());
    }

    #[test]
    fn vad_caps_utterance_at_thirty_seconds() {
        let mut vad = Vad::new(RATE);
        let frames_30s = MAX_UTTERANCE_SECONDS * 1000 / FRAME_MS;
        // O corte acontece mesmo sem silêncio algum.
        let utterance = feed(&mut vad, &speech_frame(), frames_30s).expect("corte por tamanho");
        assert_eq!(utterance.len(), vad.max_samples);
    }

    #[test]
    fn wav_header_describes_mono_pcm_16() {
        let wav = encode_wav(&[1, -2, 3], 24_000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes([wav[20], wav[21]]), 1); // PCM
        assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 1); // mono
        assert_eq!(
            u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]),
            24_000
        );
        assert_eq!(u16::from_le_bytes([wav[34], wav[35]]), 16); // bits
        assert_eq!(u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]), 6);
        assert_eq!(&wav[44..], &[1, 0, 0xfe, 0xff, 3, 0]);
    }

    #[test]
    fn sample_conversions_cover_signed_and_unsigned_formats() {
        assert_eq!(f32_to_i16(1.0), i16::MAX);
        assert_eq!(f32_to_i16(-1.0), -i16::MAX);
        assert_eq!(f32_to_i16(2.0), i16::MAX);
        assert_eq!(f64_to_i16(0.5), 16_383);
        assert_eq!(u16_to_i16(0), -32_768);
        assert_eq!(u16_to_i16(65_535), 32_767);
    }

    #[test]
    fn gate_starts_suspended_and_splits_user_from_agent() {
        let gate = MicGate::new();
        assert!(gate.suspended());
        gate.set_user(false);
        assert!(!gate.suspended());
        gate.set_agent(true);
        assert!(gate.suspended());
        gate.set_agent(false);
        assert!(!gate.suspended());
    }
}
