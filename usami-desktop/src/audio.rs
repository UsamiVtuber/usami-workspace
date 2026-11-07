use std::io::Cursor;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

pub struct AudioPlayer {
    _sink: MixerDeviceSink,
    player: Player,
}

impl AudioPlayer {
    pub fn try_new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let sink = DeviceSinkBuilder::open_default_sink()
            .map_err(|e| format!("Falha ao abrir dispositivo de áudio padrão: {e}"))?;
        let player = Player::connect_new(&sink.mixer());
        Ok(Self {
            _sink: sink,
            player,
        })
    }

    pub fn play_wav(&self, wav_bytes: Vec<u8>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let cursor = Cursor::new(wav_bytes);
        let source = Decoder::try_from(cursor)
            .map_err(|e| format!("Falha ao decodificar áudio WAV: {e}"))?;
        self.player.append(source);
        Ok(())
    }
}
