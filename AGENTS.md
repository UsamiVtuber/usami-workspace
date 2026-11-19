# Instruções para agentes

Este workspace reúne projetos diferentes; identifique o componente da tarefa antes de alterar arquivos. Use as instruções mais próximas dos arquivos como complemento deste guia.

## Orientação pelo repositório

- `usami/` é o runtime principal da companion e tem Git e instruções próprios. Antes de trabalhar nele, leia [`usami/AGENTS.md`](usami/AGENTS.md), confira o estado desse repositório separadamente e consulte [`usami/docs/architecture.md`](usami/docs/architecture.md). As regras específicas do protótipo e as decisões de produto em `usami/AGENTS.md` continuam valendo.
- `usami-desktop/` é o cliente desktop (GPUI/Rust). Antes de trabalhar nele, leia [`usami-desktop/docs/KNOWN_ERRORS.md`](usami-desktop/docs/KNOWN_ERRORS.md) para conhecer erros, falhas e correções já registrados e evitar regressões.
- `infra/n8n-local/` contém a configuração local do n8n/PostgreSQL e os exports de workflows. Consulte seus `README.md` antes de operar ou alterar a stack; o guia de workflows está em [`infra/n8n-local/workflows/README.md`](infra/n8n-local/workflows/README.md).
- `usami-monitor/` contém um painel FastAPI e frontend estático, destinado ao notebook Linux. Consulte [`usami-monitor/README.md`](usami-monitor/README.md) para implantação e restrições de acesso.
- `vault/` contém dados locais do Obsidian. Trate seu conteúdo como privado: não faça buscas, leituras em lote, alterações ou exclusões sem necessidade direta para a tarefa. Para integrações, limite gravações às pastas de automação explicitamente documentadas e preserve o restante do vault.
- `assets/` pode conter material de terceiros e arquivos grandes; não copie, modifique ou versione esses arquivos sem necessidade.
- `Agents/Skills/` contém skills. Consulte apenas a skill aplicável à tarefa e leia seu `SKILL.md` antes de segui-la; não varra nem carregue o catálogo inteiro.

O `README.md` da raiz descreve o workspace em alto nível. Para detalhes de um componente, prefira a documentação e as instruções mantidas dentro dele; confirme o estado atual no código em vez de presumir que descrições antigas continuam corretas.

## Fluxo de trabalho

1. Leia este arquivo e os `AGENTS.md` aplicáveis até a pasta do componente. Antes de editar, confira `git status` na raiz e, para `usami/`, também dentro desse repositório. Preserve alterações existentes.
2. Examine a documentação, o código, os testes e as configurações diretamente relacionados à tarefa. Siga os contratos e padrões já usados pelo componente.
3. Faça a menor mudança completa que resolva o pedido. Evite refatorações, dependências e alterações fora do escopo.
4. Execute as verificações apropriadas disponíveis no componente e informe os comandos e resultados reais. Na aplicação `usami/`, os scripts ficam em `package.json` (por exemplo, `npm test`, `npm run build` e testes específicos). Não assuma que um script existe em outro componente.
5. Revise o diff para confirmar que só os arquivos esperados foram alterados.

## Interface

- Não use emojis em código, interface ou documentação do projeto. A interface do desktop usa os ícones SVG de [`usami-desktop/assets/icons/`](usami-desktop/assets/icons); quando faltar um símbolo, adicione um SVG novo em vez de um emoji.

## Segurança e operações locais

- Nunca leia, copie, imprima ou versione credenciais. Não inclua valores de `.env`, chaves, tokens ou dados pessoais em código, logs ou respostas.
- Não faça operações destrutivas em dados locais, no vault, em backups ou em volumes Docker. Em particular, não remova volumes do n8n; confirme o alvo e peça autorização antes de qualquer ação irreversível.
- Preserve a exposição local prevista pela documentação: não abra portas, publique serviços, altere túneis nem envie dados para serviços externos sem autorização explícita.
- Não teste sistemas ou contas sem autorização, nem desative testes ou controles para mascarar falhas.

## Ao concluir

Resuma a mudança e os arquivos afetados, indique as verificações executadas (ou não executadas) e registre limitações ou riscos que permaneçam. Não declare sucesso sem evidência.
