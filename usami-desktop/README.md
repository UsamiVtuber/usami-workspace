# Usami Desktop (Rust + GPUI)

Aplicativo nativo de alta performance para a companion Usami no desktop, utilizando **Rust** e o framework **GPUI** (da Zed Industries).

## Vantagens em relação à Web/Electron
- **Consumo de memória**: ~35–60 MB de RAM (contra 400+ MB no Chromium).
- **Tempo de inicialização**: Praticamente instantâneo (< 200 ms).
- **Áudio de baixa latência**: Reprodução direta de áudio PCM/WAV via `rodio` e `cpal`.
- **Renderização acelerada por GPU**: Renderização direta via DirectX/Direct3D/Vulkan através da stack do GPUI.

## Estrutura do Projeto
- `src/main.rs`: Janela principal GPUI com tema nativo escuro/roxo da Usami e palco 3D interativo alimentado pela crate `gpui-vrm` (`VrmView` + `VrmController`).
- `src/api.rs`: Cliente assíncrono para os endpoints da Usami (`/api/chat` e `/api/tts`).
- `src/audio.rs`: Mecanismo de áudio de alta fidelidade para reprodução sem pausas de Garbage Collection.
- `../gpui-vrm`: Motor 3D VRM/glTF/VRMA nativo em Rust (WGPU MToon Cel-Shading, SpringBone Physics, BlendShapes, Lip-Sync e Look-At IK).

## Como compilar e executar
```powershell
cd usami-desktop
cargo run
```
