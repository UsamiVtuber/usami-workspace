# Usami Contracts

Contratos versionados usados na comunicação entre o frontend, o backend e o
serviço de IA da Usami.

## Conteúdo

- `openapi/`: contratos HTTP públicos e internos;
- `schemas/`: schemas reutilizáveis de request, response e domínio;
- `events/`: eventos assíncronos e suas versões;
- `examples/`: exemplos não sensíveis para desenvolvimento e testes.

## Regras

- contratos são versionados e revisados antes de mudanças incompatíveis;
- este repositório contém schemas e documentação, não lógica de negócio;
- nenhum segredo, token, dado real de usuário ou prompt privado deve ser
  commitado;
- mudanças incompatíveis exigem nova versão do contrato ou período de
  compatibilidade documentado.

## Status

Repositório inicial. Os primeiros contratos serão adicionados junto com as
primeiras rotas do `Usami-Backend` e do `Usami-AI`.
