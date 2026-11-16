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

### KE-005 — Artefatos visuais nas bordas/cantos do smartphone (GPUI overflow_hidden não recorta)
- **Data:** 2026-11-15
- **Componente:** usami-desktop (src/views/phone.rs)
- **Erro / falha:** O chassis do smartphone (externo: `rounded(48px)`, padding `3.5px`) e a tela interna OLED (`rounded(44px)`, `overflow_hidden`) geram desalinhamento visual nos cantos — aparece "rebarba" ou linha fina do background do chassis vazando pelos cantos da tela interna, pois `overflow_hidden` no GPUI 0.2.2 não recorta cantos arredondados corretamente (quirk conhecido, ver KE-002).
- **Sintoma:** nos 4 cantos do aparelho, vê-se uma linha/fina do titânio escuro (`0x111317`) escapando por baixo dos cantos da tela OLED (`0x1f2c34`), quebrando a ilusão de tela curva perfeita dentro do chassi.
- **Causa raiz:** diferença entre `rounded(48px)` do chassis externo e `rounded(44px)` da tela interna com `padding(3.5px)` → raio efetivo interno = 48 - 3.5 = 44.5px, mas tela usa 44px (diferença de 0.5px). Somado ao bug do GPUI onde `overflow_hidden` não aplica clipping nos cantos arredondados, o conteúdo filho vaza.
- **Tentado e não funcionou:** 
  - Apenas confiar no `overflow_hidden` da tela interna (não recorta cantos no GPUI 0.2.2).
  - Aumentar padding do chassis para 4px (quebra proporções do Dynamic Island e status bar).
  - Remover `overflow_hidden` da tela interna (piora drasticamente).
- **Correção que funcionou (passo a passo do fix anterior):**
  1. Capturar screenshot do celular renderizado.
  2. Abrir em editor de imagem e inverter cores / aplicar "contraste negativo" (ex: `Colors → Invert` ou curve adjustment) para realçar bordas escuras sobre fundo claro.
  3. Medir em pixels a largura da "rebarba" vazando em cada canto (tipicamente 1-2px).
  4. Ajustar o `rounded` da tela interna para `px(44.5)` (ou `px(45)` se GPUI não aceita float) compensando o padding de 3.5px do chassis: `raio_interno = raio_externo - padding = 48 - 3.5 = 44.5`.
  5. Alternativamente, se GPUI só aceita inteiros: manter `rounded(44px)` na tela e reduzir padding do chassis para `px(4.0)` (alinha raio efetivo a 44px).
  6. **Fix adicional (2026-11-15):** como `overflow_hidden` não recorta cantos no GPUI 0.2.2, os filhos diretos de `phone-screen` que ocupam largura total (status bar, header, input bar, home indicator) renderizam seus backgrounds retangulares fora dos cantos arredondados da tela. Solução: adicionar `rounded_tl/tr(44.5)` no filho do topo (status bar) e `rounded_bl/br(44.5)` nos filhos da base (input bar, home indicator bar).
  7. Validar visualmente: recapturar, inverter cores, confirmar que não há vazamento nos 4 cantos.
- **Como evitar / reproduzir:** 
  - Sempre que alterar `rounded`, `padding` ou `border` do chassis/tela do smartphone, recalcular: `raio_interno_ideal = raio_externo - padding`.
  - Testar com screenshot + inversão de cores antes de commitar.
  - Não confiar apenas em `overflow_hidden` para clipping de cantos no GPUI 0.2.2.
  - Filhos diretos de `phone-screen` que tocam as bordas devem ter `rounded_*` correspondentes nos cantos que tocam.
- **Status:** resolvido (requer revalidação a cada alteração de layout do phone)
- **Referências:** KE-002 (quirks GPUI), commit 711299f (removeu placa de arco inferior que mascarava o problema)

---

**Como adicionar uma nova entrada:** copie o bloco acima, preencha com o próximo número KE disponível, descreva sintoma, causa raiz, tentativas fracassadas, correção que funcionou e referências (commit/PR/issue). Mantenha em pt-BR.
