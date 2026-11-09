# Usami — referência visual do web para o port GPUI

Documento de referência para portar a aparência do runtime web (`usami/`) para o app desktop
(`usami-desktop/`, Rust + GPUI 0.2.2, sem webview). **Nada aqui foi implementado no desktop.**
Este arquivo descreve intenção visual e medidas; a implementação em GPUI deve reproduzir o
resultado, não traduzir CSS linha a linha (não há DOM, `backdrop-filter` nem `:has()`).

## Metadados

| Item | Valor |
| --- | --- |
| Fonte da verdade | `usami/` @ branch `main`, commit `95fe4e3` (2026-11-08), árvore de trabalho limpa |
| Build capturado | build atual servido pelo Vite 6.4.2 + React 19 no momento da captura |
| Resolução das capturas | **1440 × 900 px CSS**, device pixel ratio **1**, frame inteiro (sem recorte) |
| Tema | **escuro** (paleta oficial D1) |
| Data da captura | 2026-11-08 |
| Pasta das imagens | `docs/design-ref/screenshots/` (neste repositório) |

### Decisão de tema (importante)

O pedido original previa "somente tema claro na v1". A decisão de produto registrada em
2026-11-08 substituiu esse default por **paridade com o tema escuro do web**, com fontes OFL
embutidas localmente. O web não possui tema claro: `index.html` declara `color-scheme: dark` e
`theme-color #0B132B`. Documentar um tema claro seria inventar um estado inexistente, então
**esta referência cobre apenas o tema escuro** e o tema claro fica explicitamente fora do escopo.

### Princípios visuais observados

1. **Palco primeiro**: o avatar 3D ocupa a tela inteira; a UI flutua em camadas discretas
   (header, composer, drawer, painéis) sobre o cenário.
2. **Vidro escuro**: superfícies translúcidas em azul-marinho com `backdrop-filter`, bordas de
   1 px branco a 12–22% e sombra profunda. O fundo nunca é uma cor chapada.
3. **Rosa como acento, não como fundo**: `#FFA6C9`/`#FF5E7E` aparecem em foco, seleção,
   progresso, crédito afetivo e erro (vermelho próprio).
4. **Tipografia editorial**: DM Sans para interface, Fraunces itálico para a "voz" da
   personagem, Rajdhani em caixa-alta para rótulos técnicos/status.
5. **UI que se retira**: após 4,5 s sem interação, os controles somem/quase somem; o crédito do
   modelo permanece visível (decisão D3).

---

## 1. Tokens

Os tokens abaixo vêm de `usami/src/usami.css` (`:root`). Valores `rgba(...)` são usados
literalmente nos componentes; não há tokens intermediários para todas as composições.

### 1.1 Cores base

| Token | Valor | Widget / uso |
| --- | --- | --- |
| `--bg` | `#0B132B` | Cor base da marca; também `theme-color` do documento |
| `--deep` | `#070D1F` | Fundo de `html`/`body`/palco, base das superfícies de bolha e scrims |
| `--deep-2` | `#04091A` | Declarado, **sem uso** no CSS atual — não portar ainda |
| `--panel` | `#131A33` | Fundo sólido do painel lateral de configurações (`aside`) |
| `--panel-elevated` | `#282D42` | Declarado; o valor é usado literalmente nas bolhas do chat e na barra de mídia |
| `--charcoal` | `#282D42` | Alias de `--panel-elevated` (mesmo uso literal) |
| `--surface` | `#131A33` | Alias de `--panel` (declarado, sem referência direta) |
| `--primary` | `#FFA6C9` | Rótulos ativos, títulos de seção, foco (`outline`), ícones/realces, progresso |
| `--lavender` | `#FFB8D2` | Cabeçalho "conversa", autor da bolha, pontinhos de digitação, gradiente do envio |
| `--pearl` / `--ice` | `#F9F9FB` | Texto principal e ícones; fundo dos cartões claros do painel de vínculo |
| `--pink-tint` | `#FFF1F5` | Declarado; usado literalmente no gradiente da fita de nível do vínculo |
| `--rose-tint` | `#FFE4EC` | Declarado; idem (fita e fundo do progresso de vínculo) |
| `--accent` | `#FF5E7E` | Botão/aba selecionada, gradiente do envio, cor de perigo suave |
| `--accent-2` | `#FFA6C9` | Declarado, sem uso direto (mesmo valor de `--primary`) |

### 1.2 Cores semânticas e de traço

| Token | Valor | Widget / uso |
| --- | --- | --- |
| `--line` | `rgba(249,249,251,.12)` | Borda padrão de ícones, pilha de status, cartões, divisórias |
| `--line-strong` | `rgba(249,249,251,.22)` | Borda do composer, drawer, bolha, painel, inputs |
| `--line-glow` | `rgba(255,166,201,.35)` | Declarado, sem uso direto |
| `--muted` | `rgba(249,249,251,.66)` | Texto secundário, ícones de ferramenta, autor do turno |
| `--faint` | `rgba(249,249,251,.55)` | Placeholder do composer, aviso de privacidade, disclaimer, ícone do envio desabilitado |
| `--accent-glow` | `#FFA6C9` | Borda/glow de item selecionado e `outline` de `:focus-visible` (5 usos) |
| `--ok` | `#3ECF8E` | Ponto do status em repouso; modo de chamada ativo (verde) |
| `--warn` | `#F5C451` | Ponto do status "pensando" (pulsante) e dica de EXP do vínculo |
| `--err` | `#FF6B7A` | Turno de erro, botão de desligar, ponto do status em chamada, retry |

### 1.3 Composições de superfície (glass)

| Superfície | Fundo | Borda | Blur |
| --- | --- | --- | --- |
| Ícone redondo do header | `rgba(19,26,51,.78)` | 1 px `--line` | 14 px |
| Ícone ativo (`.on`) | `rgba(255,94,126,.35)` | `--accent-glow` | 14 px |
| Pilha de status | `rgba(11,19,43,.82)` | 1 px `--line` | 14 px |
| Pilha de vínculo | gradiente 135° `rgba(255,94,126,.45) → rgba(255,166,201,.3)` | `rgba(255,166,201,.35)` | 10 px |
| Composer | `rgba(11,19,43,.72)` | 1 px `--line-strong` | 24 px |
| Drawer de conversa | `rgba(11,19,43,.82)` | 1 px `--line-strong` | 24 px |
| Bolha de fala (Usami) | `rgba(7,13,31,.85)` | 1 px `--line-strong` | 18 px |
| Turno do chat | `rgba(40,45,66,.85)` | 1 px `--line` | — |
| Turno do usuário | mesmo `rgba(40,45,66,.85)` | `rgba(255,166,201,.25)` | — |
| Turno de erro | `rgba(255,107,122,.14)` | `rgba(255,107,122,.55)` | — |
| Painel de configurações | `--panel` sólido | 1 px `--line-strong` à esquerda | — |
| Scrim do painel | `rgba(7,13,31,.6)` | — | 4 px |
| Painel de vínculo | gradiente radial rosa + `linear(160deg, rgba(40,45,66,.94), rgba(11,19,43,.96))` | 1.5 px `rgba(255,166,201,.45)` | — |
| Cartão claro do vínculo | `rgba(249,249,251,.94)` | 1 px `rgba(249,249,251,.7)` | — |
| Overlay de modelo | `rgba(11,19,43,.78)` | 1 px `--line` | 12 px |
| Caption de voz | `rgba(11,19,43,.88)` | 1 px `--line` | 16 px |
| Barra de mídia | `rgba(40,45,66,.85)` | 1 px `--primary` | 16 px |

### 1.4 Paleta local do painel de vínculo

O painel de vínculo redefine variáveis no próprio scrim porque usa cartões claros sobre o
palco escuro (estilo visual novel). Não são tokens globais.

| Variável local | Valor | Uso |
| --- | --- | --- |
| `--bond-ink` | `#232B47` | Texto sobre cartão claro |
| `--bond-ink-soft` | `#3A4466` | Texto secundário sobre cartão claro |
| `--bond-pink` | `#C43A56` | Ícones, coração, nível atual |
| `--bond-pink-soft` | `#FFA6C9` | Coração do badge (traço) |
| `--bond-violet` | `#FF7A96` | Ícones de histórico/elos intermediários |
| `--bond-card` | `rgba(249,249,251,.94)` | Fundo dos cartões |

> A fita de nível usa `linear-gradient(180deg, #FFF1F5, #FFE4EC)` com borda
> `rgba(255,94,126,.55)`; o histórico usa divisórias `rgba(255,94,126,.16)`.

### 1.5 Tipografia

| Papel | Família | Peso | Tamanho | Line-height | Tracking | Onde |
| --- | --- | --- | --- | --- | --- | --- |
| Texto de interface | DM Sans | 400 | 13 px | 1.48–1.65 | normal | Turnos do chat, parágrafos do painel, corpo |
| Texto do composer | DM Sans | 400 | 14.5 px | padrão (~1.2) | normal | `input` do composer e placeholder |
| Fala da personagem | Fraunces | 400 itálico | 16 px | 1.5 | normal | Corpo da bolha de fala |
| Título editorial | Fraunces | 500 | 32 px | padrão | normal | `h2` "Usami" no painel |
| Frase de vazio | Fraunces | itálico | 20 px | padrão | normal | "Comece do seu jeito." no chat vazio |
| Caption de voz | Fraunces | itálico | 13.5 px | 1.5 | normal | Legenda da chamada acima do composer |
| Nível (badge) | Fraunces | 600 | 30 px | padrão | normal | "Nv.1" no painel de vínculo |
| Rótulo técnico | Rajdhani | 600/700 | 11–13.5 px | padrão | .06–.22em, caixa-alta | Status, abas, títulos de seção, retry, autor do turno |
| Status central | Rajdhani | 600 | 12 px | padrão | .08em, caixa-alta | "com você", "pensando", "sua vez · 00:02" |
| Pilha de vínculo | Rajdhani | 700 | 11 px | padrão | .08em, caixa-alta | "♥ Nível 1" |
| Crédito do modelo | Noto Serif JP → DM Sans | 400 | 11 px | padrão | normal | Canto superior direito (sempre visível) |
| Aviso de privacidade | DM Sans | 400 | 11 px | padrão | normal | "Mensagens usam os provedores configurados…" |
| Dica/erro do painel | DM Sans | 400 | 11–12 px | 1.45–1.5 | normal | Hints, avisos de memória, disclaimer |
| Texto do overlay de modelo | DM Sans | 400 | 12 px | 1.5 | normal | "Carregando modelo…" / erro |

Famílias e fallbacks declarados:

```css
--font-body: 'DM Sans', system-ui, -apple-system, sans-serif;
--font-character: 'Fraunces', Georgia, serif;
--font-jp: 'Noto Serif JP', serif;
--font-display: 'Rajdhani', system-ui, sans-serif;
```

### 1.6 Espaçamento

Não existe escala de spacing em variáveis; os valores abaixo são os usados nos componentes.

| Área | Medidas |
| --- | --- |
| Header | topo 20 px; laterais 24 px; gap entre ícones 8 px; ícone 38 × 38 px |
| Canto direito | gap 10 px; crédito com 2 px de padding à direita |
| Pilha de status | padding 6 × 16 px; gap 8 px; ponto 7 px; raio 30 px |
| Controles (base) | `bottom: 20px`; largura `min(640px, 100% - 32px)`; gap vertical 9 px |
| Composer | padding `6px 8px 6px 12px`; gap 6 px; input padding `8px 6px`; botão 38 × 38 px |
| Drawer de conversa | `left: 24px`, `bottom: 84px`; largura `min(390px, 100vw - 48px)`; altura máx. `54vh` |
| Cabeçalho do drawer | padding `12px 14px 10px 16px`; gap das ações 16 px (alvos de 44 px) |
| Lista do chat | padding 14 px; gap 10 px; turno até 88% da largura; balão padding 10 × 14 px |
| Painel lateral | largura `min(420px, 100%)`; padding 32 px; cabeçalho com 20 px abaixo |
| Abas do painel | gap 6 px; padding do tab `8px 14px`; 10 px abaixo da borda |
| Seções do painel | `h3` com 24 px acima e 8 px abaixo; labels com 16 px acima / 6 px abaixo |
| Inputs do painel | padding 10 px; hint 4 px acima |
| Painel de vínculo | largura `min(580px, 100%)`; padding 18 px; gap 12 px; card padding 13 × 15 px |
| Linha do vínculo | grid `1fr 150px`, gap 12 px; elos com gap 11 px |
| Bolha de fala | largura máx. `min(88vw, 440px)`; padding `14px 20px 16px`; rabicho 14 px |
| Posição da bolha | `left: clamp(24px, 10vw, 140px)`; `top: clamp(80px, 15vh, 150px)` |
| Overlay de modelo | `top: 18%`; largura máx. `min(420px, 100% - 32px)`; padding 12 × 18 px; gap 10 px |
| Dock de chamada | gap 10 px; botão redondo 48 × 48 px |
| Caption de voz | largura máx. `min(560px, 100vw - 40px)`; padding 10 × 16 px |
| Alvos de toque | 44 × 44 px garantidos por pseudo-elemento transparente em ícones, botões do composer, ações do drawer, retry, fechar do vínculo |

### 1.7 Raios

| Raio | Widget |
| --- | --- |
| 4 px | Canto inferior-esquerdo da bolha de fala; `code` inline |
| 6 px | Botão de remover memória; badge de VRMA |
| 8 px | Inputs/selects do painel, `select` de rig, badge de expressão |
| 9 px | Abas do painel, botões de ação do painel, avisos de memória |
| 10 px | Itens da lista de memória, textarea de personalidade |
| 12 px | Botão de upload do avatar |
| 14 px | Painel do testador de VRMA |
| 16 px | Balões do chat, overlay de modelo, caption de voz, barra de mídia, painel de palco |
| 18 px | Cartões do painel de vínculo |
| 20 px | Bolha de fala, drawer de conversa, botão de histórico |
| 26 px | Painel (sheet) de vínculo |
| 28 px | Composer |
| 30 px | Pilha de status e pilha de vínculo |
| 999 px | Pílulas (retry, modelo, upload, tracks) |
| 50% | Ícones redondos, botões do composer, pontos de status, nós do vínculo |

### 1.8 Sombras

| Widget | Sombra |
| --- | --- |
| Ícone ativo | `0 0 14px rgba(255,166,201,.35)` |
| Pilha de status | `0 8px 24px rgba(0,0,0,.35)` |
| Pilha de vínculo (hover) | `0 0 12px rgba(255,166,201,.3)` |
| Composer | `0 16px 45px rgba(0,0,0,.45), 0 0 16px rgba(255,166,201,.08)` |
| Composer em foco | `0 16px 45px rgba(0,0,0,.6), 0 0 20px rgba(255,166,201,.2)` |
| Envio habilitado | `0 0 14px rgba(255,166,201,.4)` |
| Bolha de fala | `0 20px 50px rgba(0,0,0,.55), 0 0 30px rgba(255,166,201,.14)` |
| Drawer de conversa | `0 24px 60px rgba(0,0,0,.7), 0 0 25px rgba(255,166,201,.12)` |
| Painel lateral | `-20px 0 70px rgba(0,0,0,.75)` |
| Painel de vínculo | `0 30px 80px rgba(0,0,0,.6), 0 0 40px rgba(255,94,126,.22), inset 0 0 0 1px rgba(249,249,251,.04)` |
| Cartão claro do vínculo | `0 8px 22px rgba(0,0,0,.28)` |
| Nó atual do vínculo | `0 0 0 4px rgba(255,94,126,.25), 0 0 18px rgba(255,94,126,.7)` |
| Overlay de modelo | `0 12px 32px rgba(0,0,0,.45)` |
| Caption de voz | `0 10px 30px rgba(0,0,0,.45)` |
| Barra de mídia | `0 8px 24px rgba(0,0,0,.45), 0 0 12px rgba(255,166,201,.15)` |
| Botão de upload (hover) | `0 0 14px rgba(255,166,201,.3)` |
| Desligar chamada (hover) | `0 0 14px rgba(255,107,122,.5)` |

Sombras de texto: crédito `0 1px 8px rgba(0,0,0,.8)`, aviso de privacidade
`0 1px 6px rgba(0,0,0,.8)`, texto da bolha `0 1px 3px rgba(0,0,0,.4)`.

### 1.9 Opacidade, blur e textura

| Item | Valor |
| --- | --- |
| Grain de filme global | `opacity: .045`, `mix-blend-mode: overlay` (SVG fractal noise 200 × 200) |
| Header/ações/canto/vínculo em idle | `opacity: 0`, `translateY(-8px)`, sem ponteiro |
| Status em idle | `opacity: 0` |
| Controles em idle | `opacity: .15`, `translateY(8px)`; volta a 1 no hover |
| Botão do composer desabilitado | `opacity: .38` |
| Ações do cabeçalho do chat desabilitadas | `opacity: .35` |
| Botões redondos / ações do painel desabilitados | `opacity: .4` |
| Retry do chat desabilitado | `opacity: .45` |
| Corações vazios do vínculo | `rgba(249,249,251,.25)` / `rgba(255,94,126,.35)` |
| Elos bloqueados | `opacity: .6` |
| Scrim do painel | `rgba(7,13,31,.6)` + blur 4 px |
| Blur por superfície | 4 px scrim · 10 px pilha de vínculo · 12 px histórico/overlay · 14 px ícones/status · 16 px mídia/caption/painel de palco · 18 px bolha · 20 px testador VRMA · 24 px composer/drawer |

### 1.10 Movimento

| Token / animação | Valor |
| --- | --- |
| `--dur-fast` | .15 s |
| `--dur` | .2 s |
| `--dur-slow` | .3 s |
| `--ease` | `cubic-bezier(0.2, 0.8, 0.2, 1)` |
| `--ease-io` | `cubic-bezier(0.7, 0, 0.3, 1)` |
| Entrada da bolha | opacity + `translateY(10px) scale(.96)` → 1, .3 s |
| Entrada do drawer | opacity + `translateY(12px) scale(.97)` → 1, .28 s |
| Entrada do painel de vínculo | fade .25 s + rise .32 s (14 px, scale .98) |
| Header/controles | opacity + translate .4 s |
| Ponto "pensando" | pulso 1 s alternando opacity .4 → 1 e scale .85 → 1.2 |
| Digitação | 3 pontos, 1.2 s, delays 0 / .15 / .3 s, sobe 3 px |
| Spinner do modelo | 0.9 s linear infinito |
| Fechar do vínculo (hover) | gira 90° |
| `prefers-reduced-motion` | animações/transições globais reduzidas a 0.01 ms, 1 iteração |

---

## 2. Componentes

### 2.1 Header / navegação

- Três botões quadrados de 38 px com raio 50%: conversa, configurações, recentralizar.
- Ícone em `--pearl`; fundo `rgba(19,26,51,.78)`; borda `--line`.
- **Hover**: fundo `rgba(255,166,201,.22)`, borda `--line-strong`; **ativo/selecionado**:
  fundo `rgba(255,94,126,.35)`, borda `--accent-glow`, glow `0 0 14px rgba(255,166,201,.35)`.
- **Focus-visible** (global): `outline: 2px solid #FFA6C9; outline-offset: 2px`.
- À direita: botão do testador VRMA, pilha de vínculo e crédito.
- Crédito sempre visível (D3), `Noto Serif JP` 11 px, `--muted`, com versão curta no mobile.

### 2.2 Status central

- Pílula de 12 px Rajdhani 600, caixa-alta, tracking .08em, padding 6 × 16, raio 30.
- Conteúdo real: `com você` (repouso, ponto `--ok`), `pensando` (ponto `--warn` pulsante),
  `modo companion` (repouso silencioso), em chamada `sua vez · MM:SS` (ponto `--err`) ou
  `Usami está pensando` (ponto `--warn`). Avisos de voz substituem o texto temporariamente.
- Fica centralizado no topo; em ≤700 px entra no grid do header e sofre elipse.

### 2.3 Pilha de vínculo

- Rajdhani 700 11 px caixa-alta, "♥ Nível N", raio 30, padding 7 × 14.
- Gradiente rosa translúcido + blur 10; hover intensifica o gradiente e adiciona glow.
- Abre o painel de vínculo; não é um indicador de nível isolado.

### 2.4 Composer

- Largura máx. 640 px, raio 28, borda `--line-strong`, fundo `rgba(11,19,43,.72)`.
- Estrutura: botão de mídia (38) → input flexível (14.5 px) → telefone → voz → dormir → enviar.
- **Foco**: borda `--primary` + glow rosa (`0 0 20px rgba(255,166,201,.2)`).
- **Enviar habilitado**: gradiente 135° `#FF5E7E → #FFB8D2` com glow; **desabilitado**:
  fundo `rgba(249,249,251,.08)`, ícone `--faint`, opacidade .38.
- Ferramentas em `--muted`, hover `rgba(249,249,251,.14)`; mídia selecionada fica
  `--primary` com fundo `rgba(255,166,201,.2)`.
- Chamada ativa no compositor: verde (`rgba(62,207,142,.25)`); voz mutada: rosa
  (`rgba(255,138,150,.12)` com texto `rgba(255,138,150,.85)`); o botão de dormir usa o
  mesmo verde quando o Companion está ativo e rosa (`on`) quando o avatar está apenas
  dormindo (`quiet`).
- Acima do composer: barra de mídia (quando há arquivo) e o aviso de privacidade 11 px.

### 2.5 Bolha de fala (voz)

- Máx. 440 px, raio 20 com canto inferior-esquerdo 4 e rabicho de 14 px.
- Fundo `rgba(7,13,31,.85)`, borda `--line-strong`, blur 18, sombra com glow rosa.
- Cabeçalho "USAMI · ALEGRE" em Rajdhani 700 11 px .14em, cor `--lavender`/`--primary`.
- Texto em Fraunces itálico 16 px, line-height 1.5.
- Entra com fade + subida; clicar dispensa.

### 2.6 Drawer de conversa

- 390 px de largura, máx. 54vh, raio 20, fundo `rgba(11,19,43,.82)`, blur 24.
- Cabeçalho: ícone + "CONVERSA" (Rajdhani 700 11.5 px .12em, `--lavender`) e ações
  exportar/limpar/fechar (28 px visuais, 44 px de alvo; desabilitadas a .35).
- Turnos: autor em Rajdhani 11 px caixa-alta (usuário `--primary`, Usami `--muted`);
  balão 13 px, raio 16, padding 10 × 14; usuário alinhado à direita com borda rosa.
- **Vazio**: "Comece do seu jeito." (Fraunces itálico 20) + "Escreva abaixo ou inicie uma
  chamada com a Usami." (12 px).
- **Carregando**: três pontos `--lavender` (5 px) em balão próprio; status vira "pensando".
- **Erro**: balão `rgba(255,107,122,.14)` com borda vermelha, autor `--err`, retry em pílula
  11 px Rajdhani 700 com borda vermelha.
- O turno de voz recebe sufixo "· voz" no autor.
- Histórico persistido localmente (últimos 100 turnos).

### 2.7 Painel de configurações

- Scrim com blur 4 + painel sólido de 420 px à direita, raio 0, sombra à esquerda.
- Cabeçalho: "CONFIGURAÇÕES" (Rajdhani 11 px .22em, `--primary`) + "Usami" (Fraunces 32/500)
  e fechar 38 px.
- Abas reais: Conversa, Voz, Avatar, Memória, Jeito, Sobre. Rajdhani 600 11.5 px .06em,
  raio 9, padding 8 × 14; ativa em rosa translúcido + glow. A faixa rola horizontalmente
  quando não cabe (na captura de 420 px, "Sobre" fica fora do quadro).
- Conteúdo: `h3` Rajdhani 700 13.5 px .08em `--primary`; parágrafos 13 px `--muted` 1.65;
  labels 12 px; inputs/selects fundo `rgba(19,26,51,.9)`, borda `--line-strong`, raio 8,
  padding 10; botões de ação 12 px, raio 9, fundo `rgba(40,45,66,.75)`; dicas 11 px.
- Ações destrutivas usam fundo `rgba(209,72,90,.28–.35)` com texto `#FFE1E6`.

### 2.8 Painel de vínculo

- Sheet de 580 px com raio 26, gradiente escuro + radial rosa, borda rosa 1.5 px.
- Badge de nível 132 × 116 com coração rosa e nível em Fraunces 30/600; fita clara abaixo.
- EXP em Rajdhani 20/600; track de 12 px com preenchimento rosa e glow.
- Cartões claros `rgba(249,249,251,.94)` com raio 18: "NOSSOS VÍNCULOS" (corações + tracks
  de 7 px), nota manuscrita (Fraunces itálico 16, leve rotação 1.2°), progresso de 5 nós e
  histórico recente. Nós: 34 px, atual 42 px com halo rosa; bloqueados em `#E3E8F4`.
- Fecha com X de 34 px; scrim é gradiente da esquerda para a direita, sem blur.

### 2.9 Chamada de voz

- O composer continua ativo com placeholder "Escreva enquanto fala...".
- Acima: caption em Fraunces itálico 13.5 sobre fundo escuro.
- Dock de 5 botões redondos de 48 px: microfone (estado por cor: idle `--muted`,
  listening `#3ECF8E`, hearing `#9FB6E8`, thinking `#F5C451`, speaking `#FFA6C9`,
  error `#FF6B7A`), teclado (selecionado rosa), desligar (sólido `#FF6B7A`), voz e dormir.
- Status em chamada mostra ponto vermelho e "sua vez · MM:SS" ou "Usami está pensando".
- A bolha de fala mostra a última resposta com emoção no cabeçalho.

### 2.10 Overlay de modelo (VRM)

- Centralizado a 18% do topo, cartão de 16 px de raio com blur 12, fundo
  `rgba(11,19,43,.78)`, sem bloquear o composer.
- **Carregando**: spinner de 20 px (borda rosa 25% com topo `--primary`) + "Carregando modelo…".
- **Erro**: borda `rgba(255,107,122,.6)`, mensagem em `--pearl` e pílula "RECARREGAR MODELO"
  em Rajdhani 700 11 px, fundo `rgba(255,107,122,.2)`.

---

## 3. Inventário de telas e estados (somente o que existe)

### 3.1 Coberto pelas capturas

| Estado | Onde no web | Captura |
| --- | --- | --- |
| Palco em repouso, avatar idle, composer vazio, envio desabilitado | tela inicial | `01-stage-idle.png` |
| Composer com foco, texto digitado, envio habilitado | foco no input | `02-composer-focus.png` |
| Hover em ícone de navegação | header | `03-nav-hover.png` |
| Foco visível por teclado em ícone de navegação | header | `04-nav-focus-visible.png` |
| Composer focado e vazio, envio desabilitado | foco sem texto | `05-composer-disabled.png` |
| Chat vazio, exportar/limpar desabilitados | drawer | `06-chat-empty.png` |
| Conversa com turnos de usuário e Usami | drawer | `07-chat-conversation.png` |
| Chat carregando (digitação) + status "pensando" | drawer + header | `08-chat-loading.png` |
| Chat com erro + "Tentar de novo" | drawer | `09-chat-error.png` |
| Painel de configurações, aba Conversa | scrim + aside | `10-settings-panel.png` |
| Painel de vínculo (nível, EXP, relações, progresso) | sheet | `11-bond-panel.png` |
| Modo chamada: status, caption, dock, botões selecionados | chamada | `12-call-dock.png` |
| Modelo VRM carregando (overlay) | palco | `13-model-loading.png` |
| Modelo VRM com erro + recarregar | palco | `14-model-error.png` |

### 3.2 Existente no web, fora do escopo desta rodada

Não foram capturados porque não fazem parte dos quatro blocos pedidos (tokens, composer, chat,
navegação/status) ou exigiriam dados/arquivos reais. Não invente equivalentes no desktop antes
de uma decisão de produto:

- Barra de pré-visualização de mídia (imagem/vídeo) e análise associada.
- Testador de animações VRMA (painel de 310 px com clipes e sliders).
- Abas Voz, Avatar, Memória, Jeito e Sobre do painel; inclui upload de VRM, sliders de rig,
  expressões, lista de memórias, avisos de erro/sucesso e confirmação de exclusão em dois passos.
- Estados de memória vazia/erro/aviso e ações destrutivas com confirmação.
- Auto-ocultação da UI após 4,5 s de inatividade (comportamento observado no código; as
  capturas mantêm a UI ativa para documentar os controles).
- Layout responsivo em ≤900 px e ≤700 px; a referência é 1440 × 900.
- `prefers-reduced-motion` (regra global existe, não há captura estática que a demonstre).
- Fases do microfone na chamada (as cores existem; a captura mostra o estado de escuta).
- Tema claro — **não existe** no web.

---

## 4. Índice das capturas

Todas em 1440 × 900, DPR 1, frame inteiro. Conteúdo sintético e neutro; nenhuma mensagem
pessoal, credencial ou caminho interno.

| Arquivo | O que mostra |
| --- | --- |
| `01-stage-idle.png` | Palco completo, avatar idle, header/status/pilha/crédito, composer vazio, aviso de privacidade |
| `02-composer-focus.png` | Composer com borda rosa de foco, texto "Oi, Usami! Como você está hoje?" e envio em gradiente |
| `03-nav-hover.png` | Ícone de configurações em hover (fundo rosa 22%, borda forte) |
| `04-nav-focus-visible.png` | Outline rosa de 2 px no botão "Mostrar conversa" via teclado |
| `05-composer-disabled.png` | Composer focado e vazio; envio em .38 com ícone `--faint` |
| `06-chat-empty.png` | Drawer vazio com "Comece do seu jeito."; exportar/limpar .35 |
| `07-chat-conversation.png` | Quatro turnos sintéticos, autores e balões de usuário/Usami |
| `08-chat-loading.png` | Turno novo + três pontos de digitação; status "PENSANDO" com ponto âmbar |
| `09-chat-error.png` | Turno de erro em vermelho, autor em `--err`, pílula "TENTAR DE NOVO" |
| `10-settings-panel.png` | Scrim com blur, painel de 420 px, abas, histórico, intimidade, medidores |
| `11-bond-panel.png` | Sheet de vínculo com nível 1, EXP, relações, progresso de 5 níveis e histórico |
| `12-call-dock.png` | Chamada: ponto vermelho, "SUA VEZ · 00:02", bolha com emoção, caption, dock de 5 botões |
| `13-model-loading.png` | Overlay central com spinner e "Carregando modelo…" sobre o cenário sem avatar |
| `14-model-error.png` | Overlay de erro com borda vermelha e "RECARREGAR MODELO" |

Observação: a pasta também contém arquivos auxiliares com prefixo `_` (recortes/zooms de
trabalho). Eles **não** fazem parte da referência e não são referenciados por este documento.

---

## 5. Tipografia: licenças e uso no desktop

| Fonte | Uso no web | Licença | Observação para o desktop |
| --- | --- | --- | --- |
| DM Sans | Interface, corpo, composer | SIL Open Font License 1.1 | Embutir localmente (pesos 400/500/600/700) |
| Fraunces | Voz da personagem, títulos editoriais | SIL Open Font License 1.1 | Embutir localmente (400/500/600 + itálico) |
| Rajdhani | Status, rótulos, abas | SIL Open Font License 1.1 | Embutir localmente (400–700) |
| Noto Serif JP | Crédito do modelo (白ー) | SIL Open Font License 1.1 | Pode usar fallback de serif japonesa do sistema; o crédito é a única ocorrência |

O web carrega as fontes via `@import` do Google Fonts. O desktop **não deve fazer requisição
externa em runtime**: empacotar os arquivos OFL e registrar as famílias no GPUI. Nenhuma das
quatro fontes exige atribuição visível além da licença no pacote.

---

## 6. Como as capturas foram produzidas

- Backend local iniciado com `npm run api` (dentro de `usami/`) e app servido pelo Vite com
  `npm run dev`; ambos verificados com resposta 200 antes da captura. Um 500 `text/plain` do
  proxy seria upstream fora do ar — não ocorreu na rodada final.
- Navegador Chrome 156 **headless**, controlado por CDP (sem Playwright), com
  `--force-device-scale-factor=1 --hide-scrollbars --enable-unsafe-swiftshader
  --use-angle=swiftshader` para renderizar o WebGL/VRM no ambiente sem GPU visível.
- Viewport fixo de 1440 × 900 via `Emulation.setDeviceMetricsOverride`; screenshot com
  `Page.captureScreenshot` e clip exato de 1440 × 900, DPR 1. As dimensões de cada PNG foram
  validadas no próprio script.
- Conversa sintética injetada no armazenamento local antes da captura (4 turnos neutros).
- Estados de carregamento/erro do chat e do modelo foram produzidos interceptando as
  requisições no CDP (requisição presa / resposta de erro), sem alterar o código do web.
- O modo chamada usou um `SpeechRecognition` simulado e voz mutada, para capturar o dock sem
  microfone real e sem áudio.
- O script de captura ficou **fora** dos dois repositórios; nenhum arquivo de `usami/` foi
  editado, e nada foi commitado ou enviado.

---

## 7. Mapa rápido componente → tokens

| Componente GPUI | Tokens principais |
| --- | --- |
| Fundo do palco | `--deep`, grain .045, cenário próprio do modelo |
| Header | `--pearl`, `--line`, `rgba(19,26,51,.78)`, raio 50%, blur 14, foco `#FFA6C9` 2 px |
| Status | `--pearl`, `rgba(11,19,43,.82)`, `--line`, `--ok`/`--warn`/`--err`, raio 30, Rajdhani 12/600 |
| Pilha de vínculo | gradiente rosa, `rgba(255,166,201,.35)`, raio 30, Rajdhani 11/700 |
| Crédito | `--muted`, Noto Serif JP 11 |
| Composer | `rgba(11,19,43,.72)`, `--line-strong`, raio 28, blur 24, `--pearl`, `--faint`, `--primary` no foco |
| Envio | gradiente `#FF5E7E → #FFB8D2`, glow rosa; desabilitado `.38` |
| Bolha | `rgba(7,13,31,.85)`, `--line-strong`, raio 20/4, Fraunces 16 itálico, `--lavender` |
| Drawer | `rgba(11,19,43,.82)`, raio 20, blur 24, `--lavender`, `--muted`, `--primary` |
| Turno | `rgba(40,45,66,.85)`, `--line`, raio 16, 13 px/1.48 |
| Erro | `--err`, `rgba(255,107,122,.14/.55)` |
| Painel | `--panel`, `--line-strong`, `--primary`, Fraunces 32, Rajdhani 11.5 |
| Vínculo (sheet) | paleta `--bond-*`, raio 26, cartões claros, glow rosa |
| Chamada | `--ok`/`--err`/`--warn`, `#FF6B7A`, dock 48 px, caption Fraunces 13.5 |
| Overlay de modelo | `rgba(11,19,43,.78)`, blur 12, raio 16, `--err` no erro |

---

## 8. Notas para o port GPUI

1. **Não copiar o CSS**: reproduzir medidas, cores e hierarquia. GPUI não tem
   `backdrop-filter`; simular vidro com preenchimento translúcido e, se possível, blur de
   camada única — ou aceitar superfície mais opaca sem perder a leitura.
2. **Foco e acessibilidade**: manter outline rosa de 2 px com 2 px de offset e alvos mínimos
   de 44 px, mesmo que o desenho visível seja menor.
3. **Janela**: a referência é 1440 × 900. Abaixo de ~900 px de largura, o web muda o header
   para grid e estreita o drawer; o desktop deve decidir breakpoints próprios.
4. **Idle**: o web esconde header/status e deixa o composer a 15% após 4,5 s. Se adotado no
   desktop, o crédito do modelo deve permanecer visível.
5. **Tema único**: implementar apenas o escuro nesta fase; não criar variante clara sem uma
   fonte de design correspondente.
