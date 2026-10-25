# Usami Backend

API principal da Usami.

## Responsabilidades

- autenticação, identidade e permissões;
- contas, sessões e preferências de produto;
- API pública consumida pelo cliente;
- coordenação das chamadas ao `Usami-AI`;
- billing, limites de uso e integrações de produto;
- persistência operacional e auditoria de ações.

Personalidade, memória, RAG e seleção de modelos pertencem ao `Usami-AI`.
Contratos de comunicação devem ser mantidos no repositório `Usami-Contracts`.

## Status

Este commit recebe a fatia estável da API HTTP e do pipeline de TTS existente
até `cdee573` no protótipo original. STT/Whisper, memória, afinidade e o fluxo
estilo n8n continuam em desenvolvimento e foram deliberadamente deixados de
fora desta importação.
