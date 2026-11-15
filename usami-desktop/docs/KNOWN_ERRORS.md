# Known Errors — usami-desktop

Registro de erros, falhas e correções já encontradas no componente `usami-desktop`. Qualquer agente (ou pessoa) que for mexer no código deve ler este arquivo **antes** de implementar ou alterar algo, para não reencontrar o mesmo buraco nem propor correções já descartadas.

Formato de cada entrada:

```
### KE-NNN — <título curto>
- **Data:** AAAA-MM-DD
- **Componente:** usami-desktop (ou outro)
- **Erro / falha:** o que ocorreu.
- **Sintoma:** como se manifesta (log, travamento, comportamento inesperado).
- **Causa raiz:** por que ocorreu.
- **Tentado e não funcionou:** o que já se tentou e falhou.
- **Correção que funcionou:** o que de fato resolveu.
- **Como evitar / reproduzir:** passos para recriar ou guarda-redes.
- **Status:** resolvido / aberto / monitorando
- **Referências:** commits, PRs, issues relacionados
```

---

### KE-001 — Corrida na fila de comandos do n8n consome comandos de outras abas
- **Data:** 2026-11-08
- **Componente:** infra/n8n-local (afeta fluxo do desktop)
- **Erro / falha:** comandos enviados por uma aba eram consumidos por outra aba; consumidor não respeitava fila própria por aba.
- **Sintoma:** comandos desapareciam ou eram executados no contexto errado; consumidor parecia não estar consumindo.
- **Causa raiz:** fila de comandos era compartilhada entre abas; não havia ack por aba.
- **Tentado e não funcionou:** usar gesto hold para provar consumidor (não resolveu, apenas mascarava).
- **Correção que funcionou:** cada aba passou a ter fila própria + ack individual.
- **Como evitar / reproduzir:** não reintroduzir fila compartilhada entre abas; sempre validar com múltiplas abas abertas simultaneamente.
- **Status:** resolvido
- **Referências:** project-n8n-command-queue-race.md

### KE-002 — Quirks do GPUI 0.2.2 no desktop (print/overlay/texto)
- **Data:** 2026-10-27
- **Componente:** usami-desktop (GPUI)
- **Erro / falha:** comportamentos inesperados de renderização e interação na versão 0.2.2 do GPUI.
- **Sintoma:**
  - Scroll só funciona com `.id()`;
  - `overflow_hidden` não recorta cantos;
  - Rodinha do mouse atravessa overlay sem `.occlude()`;
  - Texto sem largura definida mede 1 linha (até dentro de scroll) e sobrepõe outros elementos;
  - Caps no pitch da linha;
  - Sem suporte a `text_shadow`.
- **Causa raiz:** limitações conhecidas da versão 0.2.2 do GPUI.
- **Tentado e não funcionou:** confiar em comportamento padrão de overflow/texto sem largura explícita.
- **Correção que funcionou:** aplicar `.id()` em scroll, `.occlude()` em overlays, definir largura explícita em texto dentro de scroll, evitar `text_shadow`.
- **Como evitar / reproduzir:** verificar estas regras sempre que mexer em layout/texto do desktop.
- **Status:** documentado (limitação da versão, aguardar upgrade do GPUI)
- **Referências:** gpui-0.2.2-quirks.md

### KE-003 — Armadilhas do PowerShell 5.1 no script de release
- **Data:** 2026-11-10
- **Componente:** usami-desktop (scripts/release.ps1)
- **Erro / falha:** `release.ps1` se comporta incorretamente no PowerShell 5.1 (Windows antigo/Hostinger).
- **Sintoma:** release falha ou produz saída inesperada em PS 5.1.
- **Causa raiz:** incompatibilidades de sintaxe/cmdlets entre PS 5.1 e versões modernas.
- **Tentado e não funcionou:** assumir que o script roda igual em qualquer PowerShell.
- **Correção que funcionou:** validar o script especificamente em PS 5.1 antes de publicar; evitar recursos exclusivos do PS 7+.
- **Como evitar / reproduzir:** testar release.ps1 em PS 5.1 antes de qualquer release.
- **Status:** documentado
- **Referências:** project-desktop-release-channel.md

### KE-004 — Exposição de senha do Postgres no histórico do Git
- **Data:** 2026-11-08
- **Componente:** infra/n8n-local (compose/.env)
- **Erro / falha:** senha do Postgres estava em texto claro no `.env` e foi versionada; commit antigo ainda acessível no GitHub por SHA.
- **Sintoma:** credencial exposta no histórico do Git.
- **Causa raiz:** `.env` parametrizado mas com valor fixo commiteado; histórico não reescrito completamente.
- **Tentado e não funcionar:** não se aplica (decisão de risco aceita).
- **Correção que funcionou:** compose/.env parametrizado; histórico reescrito; SHA antigo segue acessível mas o Lucas aceitou o risco.
- **Como evitar / reproduzir:** nunca commitear `.env` com valores reais; usar placeholders e variáveis de ambiente.
- **Status:** resolvido (risco aceito pelo usuário)
- **Referências:** project-postgres-secret-exposure.md

---

**Como adicionar uma nova entrada:** copie o bloco acima, preencha com o próximo número KE disponível, descreva sintoma, causa raiz, tentativas fracassadas, correção que funcionou e referências (commit/PR/issue). Mantenha em pt-BR.
