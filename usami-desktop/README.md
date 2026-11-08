# Usami Desktop (Rust + GPUI)

Aplicativo nativo de alta performance para a companion Usami no desktop, utilizando **Rust** e o framework **GPUI** (da Zed Industries).

## Vantagens em relação à Web/Electron
- **Consumo de memória**: ~35–60 MB de RAM (contra 400+ MB no Chromium).
- **Tempo de inicialização**: Praticamente instantâneo (< 200 ms).
- **Áudio de baixa latência**: Reprodução direta de áudio PCM/WAV via `rodio` e `cpal`.
- **Renderização acelerada por GPU**: Renderização direta via DirectX/Direct3D/Vulkan através da stack do GPUI.

## Estrutura do Projeto
- `src/main.rs`: Janela principal GPUI com tema nativo escuro/roxo da Usami e palco 3D interativo alimentado pela crate `gpui-vrm` (`VrmView` + `VrmController`).
- `src/api.rs`: Cliente assíncrono para os endpoints da Usami (`/api/chat`, `/api/tts` e `/api/stt`).
- `src/audio.rs`: Mecanismo de áudio de alta fidelidade para reprodução sem pausas de Garbage Collection.
- `src/recorder.rs`: Captura do microfone em thread própria com `cpal`, formando WAV PCM 16-bit mono (limite de 30 s) para o modo chamada.
- `../gpui-vrm`: Motor 3D VRM/glTF/VRMA nativo em Rust (WGPU MToon Cel-Shading, SpringBone Physics, BlendShapes, Lip-Sync e Look-At IK).

## Como compilar e executar
```powershell
cd usami-desktop
cargo run
```

O backend do core precisa estar no ar (`cd usami; npm run api`). Os assets são
resolvidos a partir da pasta da crate (`CARGO_MANIFEST_DIR`), então o app roda de
qualquer diretório. Variáveis opcionais:

- `USAMI_API_BASE` — base do backend (padrão `http://127.0.0.1:8787`); conexão
  com timeout de 5 s e requisição com teto de 90 s.
- `USAMI_VRM_PATH` — arquivo `.vrm` (padrão `../usami/public/models/shiro.vrm`).
- `USAMI_VRMA_DIR` — pasta de animações `.vrma` (padrão `../usami/public/animations/vrma`).
- `USAMI_BACKGROUND` — imagem de fundo (padrão `assets/background.png`).

## Modo chamada (handsfree)

Entrar em chamada abre o microfone automaticamente (indicador vermelho e
`OUVINDO` no status). A fala é segmentada por silêncio no próprio app:

1. a VAD abre a frase após ~120 ms de voz (com pré-roll de 400 ms) e a encerra
   após ~900 ms de silêncio;
2. o trecho vira WAV PCM 16-bit mono e vai para `/api/stt`;
3. o texto transcrito é enviado ao `/api/chat` sem clique. Se a Usami ainda
   estiver pensando, o texto fica no campo em vez de falar por cima.

O botão de microfone do dock liga/desliga a escuta; o botão de volume muta a
saída e também pausa a escuta. Enquanto o avatar fala, o microfone fica fechado
(a própria voz da Usami não vira transcrição). Encerrar a chamada desliga tudo.
O backend precisa expor `POST /api/stt` (multipart, campo `audio`, resposta
`{"text": "..."}`). Sem microfone disponível, o app mostra o erro no status e
segue funcionando pelo campo de texto.

## Testes
```powershell
cargo test
```
Cobre os contratos de `/api/chat`, `/api/tts` e `/api/stt` contra um servidor
HTTP local, erros HTTP, a codificação WAV do gravador e a resolução de caminhos
de assets.
