# Kyndred-Inspired Features for Usami

Analysis of Kyndred (Tauri + WebView2 AI companion) reveals several high-value features to port to Usami.

## Status (atualizado em 2026-11-01)

| Fase | Estado |
|------|--------|
| 1 Atalhos globais e bandeja | Pendente — depende de escolher Electron ou Tauri |
| 2 Janela pet | Pendente — mesma dependência |
| 3 VRMA / idles expressivos | **Arquivada** — os `.vrma` disponíveis são incompatíveis com o shiro (VRM0); ver nota na Fase 3 |
| 4 Polimento da UI de voz | Feita (mic badge, beacon, audio revive) |
| 5 Memória e personalidade | Feita, exceto perfis por usuário (descartado) e consolidação de fatos da memória |
| 6 Configurações nativas / `usami://` | Pendente — depende da Fase 1 |

---

## Phase 1: Global Hotkeys & System Tray (High Impact, Low Effort)

### 1.1 Global Shortcuts
| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+P` | Toggle panel/window visibility |
| `Ctrl+Shift+M` | Mute/unmute voice |
| `Ctrl+Shift+W` | Wake/start listening |
| `Ctrl+Shift+R` | Toggle reaction mode (co-watching) |
| `Right Ctrl` | Stop current speech |

**Implementation**: Add global shortcut registration in frontend (may need native helper via Tauri/Electron or `globalShortcut` polyfill).

### 1.2 System Tray
- Tray icon with context menu: Mute, Wake, Reaction Mode, Settings, Quit
- Sync tray state with app state (mute indicator, reaction mode indicator)

---

## Phase 2: Pet/Companion Window Mode

### 2.1 Borderless Pet Window
- Always-on-top toggle
- Click-through mode (for overlay use)
- Subtle cursor following (gaze tracking already exists)
- Workspace awareness (hide on fullscreen apps)

### 2.2 Window State Persistence
- Save/restore position, size, mode (normal/pet) in localStorage

---

## Phase 3: Enhanced Animation System

> **Arquivada (decisão do usuário).** A inspeção da página de chamada do Kyndred
> (`/app/call`) não encontrou nenhuma referência a `vrma` nos scripts: o
> personagem é carregado como `model.fbx` (FBXLoader), com VRM e Live2D como
> alternativas. Logo, VRMA não é requisito para paridade. Os `.vrma` testados
> ficam deformados porque o shiro é VRM0 e `merge-vrma.mjs` copia as trilhas
> sem retargeting; a correção seria usar `@pixiv/three-vrm-animation` em runtime.
> Alternativa mais barata: idles expressivos procedurais.

### 3.1 VRMA Clip Support (arquivado)
- Parse `.vrma` (VRM Animation) format
- Integrate with motion director alongside GLTF clips
- Kyndred clips: `dizzy_idle.vrma`, `kneeling_idle.vrma`, `sit_floor_idle.vrma`

### 3.2 Expressive Idles
- Add variant idle animations (dizzy, kneeling, sitting)
- Reaction mode posture (leaning forward, attentive)

---

## Phase 4: Voice UI Polish

### 4.1 Mic Badge
- Visual indicator when microphone is active
- Animated pulse during listening

### 4.2 Voice Beacon/Affect
- Audio-reactive orb or waveform visualization
- Emotional affect display (matches Kyndred's `voice_affect.js`)

### 4.3 Audio Revive
- Auto-recover WebAudio context on suspension
- Handle browser autoplay policies gracefully

---

## Phase 5: Memory & Personality (Backend Depth)

### 5.1 Persistent Memory System
- Conversation summarization (periodic)
- User facts/preferences extraction
- Relationship state (extend intimacy meters → richer model)

### 5.2 Personality Configuration
- Editable system prompt in settings
- Trait sliders (warmth, curiosity, playfulness, etc.)
- Per-user personality profiles

---

## Phase 6: Native Settings & Dev Tools

### 6.1 Native Settings Window
- Dedicated window (not just sidebar panel)
- Tabbed: General, Voice, Avatar, Memory, Advanced

### 6.2 Edit Mode
- Live personality tweaking
- Animation testing panel
- Debug console access

### 6.3 URI Scheme
- `usami://settings`
- `usami://new-chat`
- `usami://toggle-pet`
- `usami://mute`

---

## Achados da inspeção web (getkyndred.com/app/call, 2026-11-01)

Leitura apenas da página e dos scripts públicos; nenhuma chamada foi iniciada
e nenhum dado de conta foi lido.

- **Stack**: Next.js (Turbopack), Three r186, `FBXLoader` para o modelo; runtime Live2D separado. Cenário é vídeo em loop (`loop-wide.webm`) + plate estático, não sala 3D.
- **STT**: `AudioWorklet` (`pcm-worklet.js`) reamostra o mic para PCM 16 kHz e envia por WebSocket ao Deepgram Flux; reamostra porque o iOS Safari ignora o `sampleRate` pedido.
- **TTS**: o servidor streama eventos tipados (`tts.chunk`, `tts.frames`, `tts.words`, `tts.visemes`, `tts.cues`, `tts.done` com `firstChunkMs`/`totalMs`). Visemas e tempos de palavras vêm prontos do servidor; o Usami deriva a boca do espectro (Gemini TTS não fornece visemas).
- **Controles da chamada**: mute, digitar, encerrar, parar a resposta, trocar sala, rewind (salvar últimos 30 s), mais controles. A chamada começa com um toque e o mic começa desligado.

Candidatos a adotar (nenhum iniciado):
1. Botão/atalho "parar a resposta" (entra junto com a Fase 1, `Right Ctrl`).
2. STT em streaming por WebSocket no lugar de `SpeechRecognition` (sem Firefox, sem nível do mic). Mudança grande.
3. Visemas/palavras vindos do TTS, só se trocarmos para um provedor que os forneça.

---

## Suggested Timeline

| Week | Phases | Deliverables |
|------|--------|--------------|
| 1 | 1-2 | Global hotkeys, system tray, pet window mode |
| 2 | 3-4 | VRMA support, expressive idles, voice UI polish |
| 3 | 5-6 | Memory system, personality config, native settings |

---

## Technical Notes

- **Frontend**: React + Vite (current) — hotkeys may need `electron` or `tauri` wrapper, or a small native Node.js helper
- **Backend**: Node.js (Usami-Backend) — memory/personality logic lives here
- **Avatar**: Three.js + @pixiv/three-vrm — VRMA parsing needed
- **Settings**: Already have panel infrastructure; promote to dedicated window

---

## Reference: Kyndred Strings Found

```
Show/Hide her    Ctrl+Shift+P
Mute mic         Ctrl+Shift+M
Stop her talking Right Ctrl
Reaction mode    Ctrl+Shift+R
Edit mode        Ctrl+Shift+E
New chat         Ctrl+Shift+W
Wake her up      Ctrl+Shift+W
Reset position
How to use her
Settings
```

Files of interest in Kyndred:
- `/pet/brain.js`, `/pet/memory.js`, `/pet/affect.js`, `/pet/stance.js`
- `/ui/voice.js`, `/ui/voice_affect.js`, `/ui/voice_beacon.js`, `/ui/mic_badge.js`
- `/ui/reaction_mode.js`, `/ui/pet_mode.js`
- `/assets/clips/*.vrma`, `/live2d/config/models.js`
- `src/pet_sense.rs`, `src/pet_windows.rs` (Rust side)