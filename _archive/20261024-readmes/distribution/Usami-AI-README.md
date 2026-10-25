# Usami AI

Serviço responsável pelo ciclo de personalização da Usami.

## Responsabilidades

- personalidade e identidade comportamental da Usami;
- preferências e contexto do usuário;
- memória de curto e longo prazo;
- ingestão, indexação e recuperação via RAG;
- montagem de contexto e prompts;
- roteamento entre provedores e modelos;
- avaliação, segurança e rastreabilidade das respostas.

O `Usami-AI` é o cérebro de personalização da plataforma. Ele não deve
assumir responsabilidades de autenticação, billing ou apresentação de UI;
essas fronteiras pertencem ao `Usami-Backend` e ao `Usami-frontend`.

## Repositórios relacionados

- `Usami-Backend`: API do produto, usuários, sessões e permissões.
- `Usami-frontend`: experiência do usuário.
- `Usami-Contracts`: contratos versionados entre os serviços.

## Status

Este commit recebe a primeira fatia estável do domínio emocional e expressivo
da Usami, extraída do protótipo original. Memória, RAG, personalidade completa
e orquestração de providers continuam em desenvolvimento e não fazem parte
desta importação.
