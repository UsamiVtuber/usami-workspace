# Usami Desktop

Aplicativo nativo (Rust + GPUI) da companion Usami para Windows: avatar VRM 3D,
chat com o backend local, TTS com lipsync, modo chamada mãos-livres, painel de
conversa em formato de celular, Central de Vínculo e Central do Agente. Com uma
partida de League of Legends aberta, ele também vira o overlay Usami.GG.

## Download

Baixe o `.zip` mais recente na aba **Releases** e descompacte em qualquer pasta.
O executável é `usami-desktop.exe`; a pasta `assets/` precisa ficar ao lado dele.

## Requisitos

- Windows 10/11 x64.
- Backend da Usami no ar em `http://127.0.0.1:8787` (`cd usami && npm run api`).
  O n8n é opcional (usado pelos workflows). Sem o backend a janela abre, mas
  chat, TTS e STT não respondem.
- `USAMI_API_TOKEN` para as rotas protegidas (transcrição, memória,
  personalidade, mídia e Central do Agente). Use o mesmo token configurado no
  core; sem ele essas telas mostram o aviso e ficam bloqueadas.
- Um modelo `.vrm` próprio e, opcionalmente, clipes `.vrma`. Esses arquivos não
  acompanham o pacote por licença; sem um modelo a cena abre sem avatar.

## Configuração

Variáveis de ambiente:

| Variável | Padrão | Descrição |
|---|---|---|
| `USAMI_API_BASE` | `http://127.0.0.1:8787` | base do backend |
| `USAMI_API_TOKEN` | — | token das rotas protegidas; aparece como `SEM TOKEN` no app quando ausente |
| `USAMI_LOL_ENABLED` | ligado | `0`, `false` ou `no` desliga o módulo Usami.GG |
| `USAMI_VRM_PATH` | `assets/shiro.vrm` ao lado do .exe | arquivo `.vrm` do avatar |
| `USAMI_VRMA_DIR` | `assets/vrma` ao lado do .exe | pasta de animações `.vrma` |
| `USAMI_BACKGROUND` | `assets/background.png` | imagem de fundo da janela |

Sem a variável de ambiente, o token também é lido de um arquivo `.env` ao lado
do `usami-desktop.exe` (linha `USAMI_API_TOKEN=...`). O executável não carrega
nenhuma chave de provedor.

Para apontar um modelo sem mexer em variáveis, basta colocar o `.vrm` em
`assets/shiro.vrm` dentro da pasta descompactada.

## Primeiro uso

1. Suba o backend (`npm run api` no repositório principal).
2. Extraia o zip e rode `usami-desktop.exe`.
3. Se o Windows mostrar "O Windows protegeu o PC", o executável não é assinado:
   use **Mais informações → Executar assim mesmo**.

O app fica na bandeja do sistema; `Ctrl+Alt+U` (ou duplo clique no ícone)
mostra/esconde a janela. A engrenagem do topo abre os Ajustes rápidos dentro do
celular; a página completa fica em Central de Vínculo → Ajustes.

O executável possui ícone nativo embutido e, em tempo de execução, o ícone da
janela, da barra de tarefas e da bandeja muda dinamicamente de acordo com o
humor e as reações da Usami em tempo real (feliz, brava, tímida, surpresa, etc.).

## Privacidade

Por padrão o app conversa com o core em `127.0.0.1:8787`; o chat, o TTS e o STT
passam por ele. O único acesso externo do próprio app é o módulo Usami.GG
(desligado com `USAMI_LOL_ENABLED=false`): ele lê a Live Client Data API local
do League of Legends (`https://127.0.0.1:2999`) e baixa os dados estáticos do
DataDragon (`ddragon.leagueoflegends.com`), guardando cache em
`%APPDATA%\Usami\lol`. A conversa fica em `%APPDATA%\Usami`; memória, humor e
vínculo ficam no core local. O token só é enviado à base configurada em
`USAMI_API_BASE` (por padrão, o backend local).
