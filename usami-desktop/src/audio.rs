use rodio::source::Source;
use rodio::{ChannelCount, Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate};
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::Duration;

/// Amostras por bloco do envelope de fala (~21 ms a 24 kHz). O callback só é
/// chamado quando um bloco é efetivamente consumido pela saída de áudio.
const LEVEL_BLOCK: usize = 512;
/// Fala típica tem RMS por volta de 0,1; o ganho deixa a boca na faixa útil.
const LEVEL_GAIN: f32 = 4.0;

pub struct AudioPlayer {
    _sink: MixerDeviceSink,
    player: Player,
}

impl AudioPlayer {
    pub fn try_new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let sink = DeviceSinkBuilder::open_default_sink()
            .map_err(|e| format!("Falha ao abrir dispositivo de áudio padrão: {e}"))?;
        let player = Player::connect_new(sink.mixer());
        Ok(Self {
            _sink: sink,
            player,
        })
    }

    pub fn play_wav(
        &self,
        wav_bytes: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let cursor = Cursor::new(wav_bytes);
        let source = Decoder::try_from(cursor)
            .map_err(|e| format!("Falha ao decodificar áudio WAV: {e}"))?;
        self.player.append(source);
        Ok(())
    }

    /// Abre uma fonte de reprodução contínua para PCM cru (Int16 LE, mono), o
    /// mesmo formato que `/api/tts` devolve em `stream:true`. O áudio começa a
    /// tocar no primeiro chunk, sem esperar a síntese inteira.
    ///
    /// `on_level` recebe o envelope (`0.0..=1.0`) do bloco que acabou de tocar,
    /// então o lip-sync acompanha o alto-falante e não a chegada da rede.
    pub fn play_pcm_stream<F>(&self, sample_rate: u32, on_level: F) -> PcmSink
    where
        F: FnMut(f32) + Send + 'static,
    {
        let sample_rate = SampleRate::new(sample_rate.clamp(8_000, 192_000))
            .unwrap_or_else(|| SampleRate::new(24_000).expect("taxa de amostragem padrão válida"));
        let channels = ChannelCount::new(1).expect("mono é um canal válido");
        let (sender, receiver) = mpsc::channel();
        let pending = Arc::new(AtomicUsize::new(0));
        self.player.append(PcmStreamSource {
            receiver,
            buffer: VecDeque::new(),
            pending: Arc::clone(&pending),
            sample_rate,
            channels,
            on_level: Box::new(on_level),
            block_sum: 0.0,
            block_len: 0,
            level_reported_end: false,
        });
        PcmSink::new(sender, pending)
    }
}

type LevelCallback = Box<dyn FnMut(f32) + Send>;

/// Fonte infinita até o canal fechar: entrega silêncio quando a rede atrasa,
/// em vez de encerrar a fala no primeiro intervalo sem amostras.
struct PcmStreamSource {
    receiver: Receiver<Vec<f32>>,
    buffer: VecDeque<f32>,
    pending: Arc<AtomicUsize>,
    sample_rate: SampleRate,
    channels: ChannelCount,
    on_level: LevelCallback,
    block_sum: f32,
    block_len: usize,
    level_reported_end: bool,
}

impl PcmStreamSource {
    fn report_level(&mut self) {
        if self.block_len == 0 {
            return;
        }
        let rms = (self.block_sum / self.block_len as f32).sqrt();
        (self.on_level)((rms * LEVEL_GAIN).clamp(0.0, 1.0));
        self.block_sum = 0.0;
        self.block_len = 0;
    }
}

impl Iterator for PcmStreamSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        loop {
            if let Some(sample) = self.buffer.pop_front() {
                self.pending.fetch_sub(1, Ordering::Relaxed);
                // Silêncio também entra no bloco: é ele que fecha a boca quando
                // o buffer esvazia no meio da fala.
                self.block_sum += sample * sample;
                self.block_len += 1;
                if self.block_len >= LEVEL_BLOCK {
                    self.report_level();
                }
                return Some(sample);
            }
            match self.receiver.try_recv() {
                Ok(samples) => self.buffer.extend(samples),
                Err(TryRecvError::Empty) => return Some(0.0),
                Err(TryRecvError::Disconnected) => {
                    if !self.level_reported_end {
                        self.level_reported_end = true;
                        self.report_level();
                        (self.on_level)(0.0);
                    }
                    return None;
                }
            }
        }
    }
}

impl Source for PcmStreamSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        self.channels
    }

    fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Escritor da fala em streaming: converte os bytes recebidos da rede em
/// amostras f32 e enfileira. `finish` fecha o canal e permite esperar o
/// buffer tocar até o fim antes de devolver o avatar ao idle.
pub struct PcmSink {
    sender: Option<Sender<Vec<f32>>>,
    pending: Arc<AtomicUsize>,
    leftover_byte: Option<u8>,
    /// Buffer reutilizado para evitar alocações no hot path de push_i16_le.
    scratch: Vec<f32>,
}

impl PcmSink {
    /// Cria um novo PcmSink com buffer de scratch pré-alocado.
    pub(crate) fn new(sender: Sender<Vec<f32>>, pending: Arc<AtomicUsize>) -> Self {
        Self {
            sender: Some(sender),
            pending,
            leftover_byte: None,
            scratch: Vec::with_capacity(4096),
        }
    }

    /// Retorna `true` quando o chunk produziu ao menos uma amostra.
    /// Reutiliza `self.scratch` para evitar alocações no hot path.
    pub fn push_i16_le(&mut self, bytes: &[u8]) -> bool {
        let Some(sender) = &self.sender else {
            return false;
        };
        if bytes.is_empty() {
            return false;
        }

        self.scratch.clear();
        let mut start = 0;
        if let Some(high) = self.leftover_byte.take() {
            self.scratch.push(i16::from_le_bytes([bytes[0], high]) as f32 / 32_768.0);
            start = 1;
        }
        let rest = &bytes[start..];
        let complete = rest.len() - rest.len() % 2;
        for pair in rest[..complete].as_chunks::<2>().0 {
            self.scratch.push(i16::from_le_bytes(*pair) as f32 / 32_768.0);
        }
        if complete < rest.len() {
            self.leftover_byte = Some(rest[rest.len() - 1]);
        }
        if self.scratch.is_empty() {
            return false;
        }

        self.pending.fetch_add(self.scratch.len(), Ordering::Relaxed);
        // Clona apenas os dados necessários para envio (evita manter borrow mutável).
        sender.send(self.scratch.clone()).is_ok()
    }

    /// Fecha a escrita. O que já foi empilhado continua tocando.
    pub fn finish(mut self) -> PcmTail {
        self.sender = None;
        PcmTail {
            pending: Arc::clone(&self.pending),
        }
    }
}

/// Restante da fala já enfileirada no rodio.
pub struct PcmTail {
    pending: Arc<AtomicUsize>,
}

impl PcmTail {
    pub fn is_drained(&self) -> bool {
        self.pending.load(Ordering::Relaxed) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_sink() -> (PcmSink, Receiver<Vec<f32>>, Arc<AtomicUsize>) {
        let (sender, receiver) = mpsc::channel();
        let pending = Arc::new(AtomicUsize::new(0));
        (
            PcmSink::new(sender, pending.clone()),
            receiver,
            pending,
        )
    }

    #[test]
    fn pcm_sink_joins_odd_chunks_into_samples() {
        let (mut sink, receiver, pending) = test_sink();

        assert!(sink.push_i16_le(&[0x00, 0x10, 0x00]));
        assert_eq!(pending.load(Ordering::Relaxed), 1);
        assert_eq!(sink.leftover_byte, Some(0x00));

        assert!(sink.push_i16_le(&[0x20, 0x00, 0x30]));
        assert_eq!(pending.load(Ordering::Relaxed), 3);
        assert_eq!(sink.leftover_byte, None);

        let samples: Vec<f32> = receiver.try_iter().flatten().collect();
        assert_eq!(samples.len(), 3);
        // 0x1000, 0x0020 e 0x3000 viram f32 normalizado no intervalo -1..1
        assert!((samples[0] - 4096.0 / 32768.0).abs() < 1e-6);
        assert!((samples[1] - 32.0 / 32768.0).abs() < 1e-6);
        assert!((samples[2] - 12288.0 / 32768.0).abs() < 1e-6);
    }

    #[test]
    fn pcm_sink_ignores_empty_chunks_and_closes_on_finish() {
        let (mut sink, receiver, pending) = test_sink();
        assert!(!sink.push_i16_le(&[]));
        assert!(sink.push_i16_le(&[0x00, 0x00, 0x00, 0x00]));

        let tail = sink.finish();
        assert!(!tail.is_drained());
        assert_eq!(pending.load(Ordering::Relaxed), 2);
        // Canal fechado: o que estava na fila continua disponível para o leitor.
        assert_eq!(receiver.try_iter().flatten().count(), 2);
    }

    fn test_source(receiver: Receiver<Vec<f32>>, pending: Arc<AtomicUsize>) -> PcmStreamSource {
        PcmStreamSource {
            receiver,
            buffer: VecDeque::new(),
            pending,
            sample_rate: SampleRate::new(24_000).unwrap(),
            channels: ChannelCount::new(1).unwrap(),
            on_level: Box::new(|_| {}),
            block_sum: 0.0,
            block_len: 0,
            level_reported_end: false,
        }
    }

    #[test]
    fn pcm_source_outputs_silence_while_empty_and_ends_on_disconnect() {
        let (sender, receiver) = mpsc::channel::<Vec<f32>>();
        let pending = Arc::new(AtomicUsize::new(0));
        let mut source = test_source(receiver, Arc::clone(&pending));

        assert_eq!(source.next(), Some(0.0));
        pending.fetch_add(2, Ordering::Relaxed);
        sender.send(vec![0.25, -0.25]).unwrap();
        assert_eq!(source.next(), Some(0.25));
        assert_eq!(source.next(), Some(-0.25));
        assert_eq!(source.next(), Some(0.0));
        drop(sender);
        assert_eq!(source.next(), None);
        assert_eq!(source.next(), None);
        assert_eq!(pending.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn pcm_source_reports_playback_level_per_block() {
        let (sender, receiver) = mpsc::channel::<Vec<f32>>();
        let pending = Arc::new(AtomicUsize::new(0));
        let levels = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut source = PcmStreamSource {
            on_level: {
                let levels = Arc::clone(&levels);
                Box::new(move |level| levels.lock().unwrap().push(level))
            },
            ..test_source(receiver, Arc::clone(&pending))
        };

        pending.fetch_add(LEVEL_BLOCK, Ordering::Relaxed);
        sender.send(vec![0.5; LEVEL_BLOCK]).unwrap();
        for _ in 0..LEVEL_BLOCK {
            assert_eq!(source.next(), Some(0.5));
        }

        let reported = levels.lock().unwrap().clone();
        assert_eq!(reported.len(), 1);
        assert!(
            (reported[0] - 1.0).abs() < 1e-6,
            "0.5 * ganho satura em 1.0"
        );

        drop(sender);
        assert_eq!(source.next(), None);
        assert_eq!(levels.lock().unwrap().last().copied(), Some(0.0));
    }
}
