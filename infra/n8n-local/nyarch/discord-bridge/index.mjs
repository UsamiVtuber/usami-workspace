import { createHmac } from 'node:crypto';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import {
  Client,
  GatewayIntentBits,
  Partials,
  PermissionFlagsBits,
  REST,
  Routes,
  SlashCommandBuilder,
} from 'discord.js';

const execFileAsync = promisify(execFile);

const DISCORD_BOT_TOKEN = process.env.DISCORD_BOT_TOKEN || '';
const DISCORD_CLIENT_ID = process.env.DISCORD_CLIENT_ID || '';
const DISCORD_GUILD_ID = process.env.DISCORD_GUILD_ID || '';
const DISCORD_CHAT_CHANNEL_ID = process.env.DISCORD_CHAT_CHANNEL_ID || '';
const DISCORD_WELCOME_CHANNEL_ID = process.env.DISCORD_WELCOME_CHANNEL_ID || '';
const DISCORD_SUGGESTIONS_CHANNEL_ID = process.env.DISCORD_SUGGESTIONS_CHANNEL_ID || '';
const DISCORD_MOD_LOG_CHANNEL_ID = process.env.DISCORD_MOD_LOG_CHANNEL_ID || '';
const DISCORD_TICKET_CATEGORY_ID = process.env.DISCORD_TICKET_CATEGORY_ID || '';
const DISCORD_OBS_ERRORS_CHANNEL_ID = process.env.DISCORD_OBS_ERRORS_CHANNEL_ID || '';
const DISCORD_OBS_SUMMARY_CHANNEL_ID = process.env.DISCORD_OBS_SUMMARY_CHANNEL_ID || '';

const N8N_CHAT_WEBHOOK = process.env.N8N_DISCORD_CHAT_WEBHOOK || 'http://n8n:5678/webhook/usami-discord-chat';
const N8N_COMMUNITY_WEBHOOK = process.env.N8N_DISCORD_COMMUNITY_WEBHOOK || 'http://n8n:5678/webhook/usami-discord-community';
const N8N_MOD_WEBHOOK = process.env.N8N_DISCORD_MOD_WEBHOOK || 'http://n8n:5678/webhook/usami-discord-mod';
const N8N_OBSERVABILITY_WEBHOOK = process.env.N8N_OBSERVABILITY_WEBHOOK || 'http://n8n:5678/webhook/usami-observability';
const N8N_WEBHOOK_SECRET = process.env.N8N_WEBHOOK_SECRET || '';

const COMMUNITY_COMMANDS = new Set(['afk', 'aniversario', 'nota', 'sugestao', 'sorteio', 'stats', 'obs']);
const MOD_COMMANDS = new Set(['ticket', 'cmd', 'sticky', 'reactionrole']);
const EPHEMERAL_COMMANDS = new Set(['nota', 'cmd', 'sticky', 'reactionrole']);

const AUDIO_EXTENSIONS = new Set(['ogg', 'mp3', 'wav', 'm4a', 'aac', 'flac', 'opus', 'weba']);
const VIDEO_EXTENSIONS = new Set(['mp4', 'webm', 'mov', 'mkv', 'avi']);
const IMAGE_EXTENSIONS = new Set(['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp']);
const TEXT_EXTENSIONS = new Set([
  'txt', 'md', 'markdown', 'json', 'csv', 'tsv', 'log', 'yaml', 'yml', 'xml', 'html', 'css',
  'js', 'mjs', 'cjs', 'ts', 'jsx', 'tsx', 'py', 'rs', 'c', 'cpp', 'h', 'hpp', 'sh',
  'bash', 'zsh', 'sql', 'toml', 'ini', 'env', 'diff', 'patch', 'dockerfile', 'r', 'java', 'kt', 'go', 'cs', 'php', 'rb', 'lua',
]);

async function extractPdfText(buffer) {
  let tmpDir = null;
  try {
    tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'discord-pdf-'));
    const tmpFile = path.join(tmpDir, 'document.pdf');
    await fs.writeFile(tmpFile, buffer);
    const { stdout } = await execFileAsync('pdftotext', ['-layout', '-enc', 'UTF-8', tmpFile, '-'], { timeout: 15000 });
    const text = (stdout || '').trim();
    console.log(`[discord-bridge] PDF extraído com sucesso (${text.length} caracteres)`);
    return text;
  } catch (err) {
    console.warn('[discord-bridge] Falha ao extrair texto do PDF com pdftotext:', redactSecrets(err?.message || err));
    return '';
  } finally {
    if (tmpDir) {
      await fs.rm(tmpDir, { recursive: true, force: true }).catch(() => { });
    }
  }
}

function guessMimeType(filename = '') {
  const ext = String(filename || '').split('.').pop()?.toLowerCase();
  switch (ext) {
    case 'png': return 'image/png';
    case 'jpg':
    case 'jpeg': return 'image/jpeg';
    case 'webp': return 'image/webp';
    case 'gif': return 'image/gif';
    case 'mp4': return 'video/mp4';
    case 'webm': return 'video/webm';
    case 'mov': return 'video/quicktime';
    case 'mp3': return 'audio/mp3';
    case 'wav': return 'audio/wav';
    case 'ogg':
    case 'opus': return 'audio/ogg';
    case 'm4a': return 'audio/m4a';
    case 'pdf': return 'application/pdf';
    default: return 'application/octet-stream';
  }
}

if (!DISCORD_BOT_TOKEN) {
  console.error('[discord-bridge] DISCORD_BOT_TOKEN ausente; defina no .env do Nyarch.');
  process.exit(1);
}

function redactSecrets(value) {
  return String(value ?? '')
    .replace(/Bearer\s+[A-Za-z0-9._-]+/gi, 'Bearer [REDACTED]')
    .replace(/(nvapi|sk|AIza)[A-Za-z0-9_-]{8,}/g, '[REDACTED_KEY]')
    .replace(/(token|secret|password|key)=([^\s&]+)/gi, '$1=[REDACTED]')
    .slice(0, 500);
}

function signPayload(timestamp, body) {
  if (!N8N_WEBHOOK_SECRET) return null;
  return 'sha256=' + createHmac('sha256', N8N_WEBHOOK_SECRET).update(`${timestamp}.${body}`).digest('hex');
}

async function emitObservabilityError(routeOrEvent, error, metadata = {}) {
  try {
    const timestamp = new Date().toISOString();
    const payload = {
      event: 'usami.observability',
      kind: 'error',
      source: 'discord-bridge',
      route: routeOrEvent,
      statusCode: 500,
      isError: true,
      errorMessage: redactSecrets(error?.message || error),
      metadata,
      timestamp,
    };
    const body = JSON.stringify(payload);
    const headers = { 'Content-Type': 'application/json', 'X-Bridge-Source': 'discord-bridge' };
    const sig = signPayload(timestamp, body);
    if (sig) {
      headers['X-Usami-Timestamp'] = timestamp;
      headers['X-Usami-Signature'] = sig;
    }
    await fetch(N8N_OBSERVABILITY_WEBHOOK, {
      method: 'POST',
      headers,
      body,
      signal: AbortSignal.timeout(4000),
    });
  } catch {
    // Silencioso para evitar loop de erro se o n8n estiver reiniciando.
  }
}

async function postToN8n(url, payload, { timeoutMs = 15000 } = {}) {
  const timestamp = new Date().toISOString();
  const body = JSON.stringify({
    ...payload,
    timestamp,
    channelsConfig: {
      chatChannelId: DISCORD_CHAT_CHANNEL_ID,
      welcomeChannelId: DISCORD_WELCOME_CHANNEL_ID,
      suggestionsChannelId: DISCORD_SUGGESTIONS_CHANNEL_ID,
      modLogChannelId: DISCORD_MOD_LOG_CHANNEL_ID,
      ticketCategoryId: DISCORD_TICKET_CATEGORY_ID,
      obsErrorsChannelId: DISCORD_OBS_ERRORS_CHANNEL_ID,
      obsSummaryChannelId: DISCORD_OBS_SUMMARY_CHANNEL_ID,
    },
  });
  const headers = {
    'Content-Type': 'application/json',
    'X-Bridge-Source': 'discord-bridge',
  };
  const sig = signPayload(timestamp, body);
  if (sig) {
    headers['X-Usami-Timestamp'] = timestamp;
    headers['X-Usami-Signature'] = sig;
  }
  const res = await fetch(url, {
    method: 'POST',
    headers,
    body,
    signal: AbortSignal.timeout(timeoutMs),
  });
  if (!res.ok) {
    throw new Error(`Webhook ${url} respondeu ${res.status}`);
  }
}

function buildSlashCommands() {
  return [
    new SlashCommandBuilder()
      .setName('afk')
      .setDescription('Ativa seu status AFK com um motivo opcional.')
      .addStringOption(opt => opt.setName('motivo').setDescription('Motivo da ausência').setRequired(false)),
    new SlashCommandBuilder()
      .setName('aniversario')
      .setDescription('Registra seu aniversário ou lista os próximos do servidor.')
      .addStringOption(opt =>
        opt.setName('acao')
          .setDescription('O que fazer')
          .setRequired(true)
          .addChoices(
            { name: 'registrar', value: 'registrar' },
            { name: 'listar', value: 'listar' },
          ))
      .addIntegerOption(opt => opt.setName('dia').setDescription('Dia (1-31)').setMinValue(1).setMaxValue(31))
      .addIntegerOption(opt => opt.setName('mes').setDescription('Mês (1-12)').setMinValue(1).setMaxValue(12)),
    new SlashCommandBuilder()
      .setName('nota')
      .setDescription('Notepad pessoal gerenciado pela Usami.')
      .addStringOption(opt =>
        opt.setName('acao')
          .setDescription('Ação da nota')
          .setRequired(true)
          .addChoices(
            { name: 'salvar', value: 'salvar' },
            { name: 'listar', value: 'listar' },
            { name: 'apagar', value: 'apagar' },
          ))
      .addStringOption(opt => opt.setName('titulo').setDescription('Título da nota ou ID para apagar'))
      .addStringOption(opt => opt.setName('conteudo').setDescription('Conteúdo da nota')),
    new SlashCommandBuilder()
      .setName('sugestao')
      .setDescription('Envia uma sugestão para votação da comunidade.')
      .addStringOption(opt => opt.setName('texto').setDescription('Descreva sua sugestão').setRequired(true)),
    new SlashCommandBuilder()
      .setName('sorteio')
      .setDescription('Inicia um sorteio (giveaway) no canal atual.')
      .addStringOption(opt => opt.setName('premio').setDescription('Prêmio do sorteio').setRequired(true))
      .addIntegerOption(opt => opt.setName('duracao_minutos').setDescription('Duração em minutos').setRequired(true).setMinValue(1).setMaxValue(10080))
      .addIntegerOption(opt => opt.setName('vencedores').setDescription('Quantidade de vencedores').setMinValue(1).setMaxValue(10)),
    new SlashCommandBuilder()
      .setName('stats')
      .setDescription('Mostra estatísticas do servidor e disponibilidade da Usami.'),
    new SlashCommandBuilder()
      .setName('obs')
      .setDescription('Resumo rápido de observabilidade das últimas 24h (APIs, erros e n8n).'),
    new SlashCommandBuilder()
      .setName('ticket')
      .setDescription('Abre ou fecha um ticket de suporte privado.')
      .addStringOption(opt =>
        opt.setName('acao')
          .setDescription('Abrir ou fechar')
          .setRequired(true)
          .addChoices(
            { name: 'abrir', value: 'abrir' },
            { name: 'fechar', value: 'fechar' },
          ))
      .addStringOption(opt => opt.setName('assunto').setDescription('Assunto do ticket')),
    new SlashCommandBuilder()
      .setName('cmd')
      .setDescription('Gerencia comandos customizados (!nome) do servidor.')
      .setDefaultMemberPermissions(PermissionFlagsBits.ManageMessages)
      .addStringOption(opt =>
        opt.setName('acao')
          .setDescription('Ação')
          .setRequired(true)
          .addChoices(
            { name: 'criar', value: 'criar' },
            { name: 'remover', value: 'remover' },
            { name: 'listar', value: 'listar' },
          ))
      .addStringOption(opt => opt.setName('nome').setDescription('Nome do comando sem !'))
      .addStringOption(opt => opt.setName('resposta').setDescription('Texto de resposta do comando')),
    new SlashCommandBuilder()
      .setName('sticky')
      .setDescription('Fixa ou remove uma sticky message no canal atual.')
      .setDefaultMemberPermissions(PermissionFlagsBits.ManageMessages)
      .addStringOption(opt =>
        opt.setName('acao')
          .setDescription('Definir ou remover')
          .setRequired(true)
          .addChoices(
            { name: 'definir', value: 'definir' },
            { name: 'remover', value: 'remover' },
          ))
      .addStringOption(opt => opt.setName('texto').setDescription('Conteúdo da sticky message')),
    new SlashCommandBuilder()
      .setName('reactionrole')
      .setDescription('Associa um emoji de mensagem a um cargo.')
      .setDefaultMemberPermissions(PermissionFlagsBits.ManageRoles)
      .addStringOption(opt => opt.setName('message_id').setDescription('ID da mensagem').setRequired(true))
      .addStringOption(opt => opt.setName('emoji').setDescription('Emoji (ex: ⭐ ou nome:id)').setRequired(true))
      .addRoleOption(opt => opt.setName('role').setDescription('Cargo a atribuir').setRequired(true)),
  ].map(cmd => cmd.toJSON());
}

async function registerSlashCommands(clientId) {
  const targetClientId = DISCORD_CLIENT_ID || clientId;
  if (!targetClientId) return;
  const rest = new REST({ version: '10' }).setToken(DISCORD_BOT_TOKEN);
  const body = buildSlashCommands();
  if (DISCORD_GUILD_ID) {
    await rest.put(Routes.applicationGuildCommands(targetClientId, DISCORD_GUILD_ID), { body });
    console.log(`[discord-bridge] ${body.length} slash commands registrados na guild ${DISCORD_GUILD_ID}.`);
  } else {
    await rest.put(Routes.applicationCommands(targetClientId), { body });
    console.log(`[discord-bridge] ${body.length} slash commands registrados globalmente.`);
  }
}

const client = new Client({
  intents: [
    GatewayIntentBits.Guilds,
    GatewayIntentBits.GuildMembers,
    GatewayIntentBits.GuildMessages,
    GatewayIntentBits.GuildMessageReactions,
    GatewayIntentBits.DirectMessages,
    GatewayIntentBits.MessageContent,
  ],
  partials: [Partials.Channel, Partials.Message, Partials.Reaction, Partials.User],
});

client.once('ready', async () => {
  console.log(`[discord-bridge] Conectado como ${client.user.tag}`);
  try {
    await registerSlashCommands(client.user.id);
  } catch (err) {
    console.error('[discord-bridge] Falha ao registrar slash commands:', redactSecrets(err?.message || err));
    await emitObservabilityError('discord.register_commands', err);
  }
});

client.on('guildMemberAdd', async member => {
  try {
    await postToN8n(N8N_COMMUNITY_WEBHOOK, {
      event: 'discord.member_join',
      guildId: member.guild.id,
      guildName: member.guild.name,
      memberCount: member.guild.memberCount,
      user: {
        id: member.user.id,
        username: member.user.username,
        globalName: member.user.globalName || member.user.username,
        avatarUrl: member.user.displayAvatarURL({ extension: 'png', size: 256 }),
      },
    });
  } catch (err) {
    await emitObservabilityError('discord.member_join', err, { guildId: member.guild.id });
  }
});

client.on('messageCreate', async message => {
  if (message.author?.bot) return;
  const isDm = !message.guildId;
  const rawContent = String(message.content || '').trim();
  const isPrefixCommand = rawContent.startsWith('!') && rawContent.length > 1;
  const mentionsBot = Boolean(client.user && message.mentions.has(client.user));
  const isChatChannel = Boolean(DISCORD_CHAT_CHANNEL_ID && message.channelId === DISCORD_CHAT_CHANNEL_ID);

  // 1. Chat & Companion Usami
  if (!isPrefixCommand && (isDm || mentionsBot || isChatChannel)) {
    let cleanContent = client.user
      ? rawContent.replace(new RegExp(`<@!?${client.user.id}>`, 'g'), '').trim()
      : rawContent;

    const attachments = message.attachments ? [...message.attachments.values()] : [];
    let media = null;

    if (attachments.length > 0) {
      const att = attachments[0];
      const mime = String(att.contentType || '').toLowerCase();
      const filename = String(att.name || '').toLowerCase();
      const ext = filename.split('.').pop() || '';

      const isImg = mime.startsWith('image/') || IMAGE_EXTENSIONS.has(ext);
      const isVid = mime.startsWith('video/') || VIDEO_EXTENSIONS.has(ext);
      const isAud = mime.startsWith('audio/') || AUDIO_EXTENSIONS.has(ext);
      const isPdf = mime === 'application/pdf' || ext === 'pdf';
      const isTxt = mime.startsWith('text/') || mime === 'application/json' || mime === 'application/xml' || TEXT_EXTENSIONS.has(ext);

      if (att.size <= 100 * 1024 * 1024) {
        if (isVid) {
          // Vídeos: envia diretamente a URL do CDN do Discord para a Usami baixar localmente,
          // evitando estourar o limite de webhook do n8n com dezenas de MBs em base64.
          media = {
            url: att.url,
            mimeType: att.contentType || guessMimeType(att.name),
            filename: att.name,
            size: att.size,
          };
        } else {
          try {
            const res = await fetch(att.url, { signal: AbortSignal.timeout(25000) });
            if (res.ok) {
              const arrayBuf = await res.arrayBuffer();
              const buffer = Buffer.from(arrayBuf);

              if (isImg) {
                media = {
                  url: att.url,
                  mimeType: att.contentType || guessMimeType(att.name),
                  data: buffer.toString('base64'),
                  filename: att.name,
                };
              } else if (isAud) {
                media = {
                  url: att.url,
                  mimeType: att.contentType || (ext === 'ogg' || ext === 'opus' ? 'audio/ogg' : (ext === 'wav' ? 'audio/wav' : 'audio/mp3')),
                  data: buffer.toString('base64'),
                  filename: att.name,
                };
              } else if (isPdf) {
                const pdfText = await extractPdfText(buffer);
                const contentToEncode = pdfText || '[PDF sem texto selecionável ou digitalizado como imagem]';
                media = {
                  url: att.url,
                  mimeType: 'text/plain',
                  data: Buffer.from(contentToEncode, 'utf8').toString('base64'),
                  filename: att.name,
                };
              } else if (isTxt) {
                const fileText = buffer.toString('utf8');
                media = {
                  url: att.url,
                  mimeType: 'text/plain',
                  data: Buffer.from(fileText, 'utf8').toString('base64'),
                  filename: att.name,
                };
              } else {
                // Outros arquivos: tenta ler como UTF-8 se for texto
                try {
                  const genericText = buffer.toString('utf8');
                  if (/^[\x09\x0A\x0D\x20-\x7E\xC2-\xF4]*$/.test(genericText.slice(0, 1000))) {
                    media = {
                      url: att.url,
                      mimeType: 'text/plain',
                      data: Buffer.from(genericText, 'utf8').toString('base64'),
                      filename: att.name,
                    };
                  } else {
                    cleanContent += `\n\n[Arquivo recebido: ${att.name} (${(att.size / 1024).toFixed(1)} KB) - formato não suportado diretamente]`;
                  }
                } catch {
                  cleanContent += `\n\n[Arquivo recebido: ${att.name}]`;
                }
              }
            }
          } catch (err) {
            console.warn('[discord-bridge] Falha ao processar anexo:', redactSecrets(err?.message || err));
            // Fallback para URL direta
            media = {
              url: att.url,
              mimeType: att.contentType || guessMimeType(att.name),
              filename: att.name,
              size: att.size,
            };
          }
        }
      }
    }

    if (cleanContent || media) {
      try {
        await message.channel.sendTyping().catch(() => { });
        await postToN8n(N8N_CHAT_WEBHOOK, {
          event: 'discord.chat_message',
          isDm,
          guildId: message.guildId || null,
          channelId: message.channelId,
          messageId: message.id,
          content: cleanContent,
          media,
          author: {
            id: message.author.id,
            username: message.author.username,
            globalName: message.author.globalName || message.author.username,
          },
        }, { timeoutMs: 60000 });
      } catch (err) {
        await emitObservabilityError('discord.chat_message', err, { channelId: message.channelId });
      }
    }
  }

  if (isDm) return;

  // 2. Engajamento (estatísticas e checagem AFK)
  const mentionedUserIds = [...message.mentions.users.values()]
    .filter(u => !u.bot && u.id !== message.author.id)
    .map(u => u.id);

  postToN8n(N8N_COMMUNITY_WEBHOOK, {
    event: 'discord.message_activity',
    guildId: message.guildId,
    channelId: message.channelId,
    messageId: message.id,
    author: {
      id: message.author.id,
      username: message.author.globalName || message.author.username,
    },
    mentionedUserIds,
  }).catch(err => emitObservabilityError('discord.message_activity', err, { guildId: message.guildId }));

  // 3. Moderação & Estrutura (Auto-Mod, !cmd customizado e Sticky Messages)
  const isMod = Boolean(message.member?.permissions?.has(PermissionFlagsBits.ManageMessages));
  const commandName = isPrefixCommand ? rawContent.slice(1).split(/\s+/)[0].toLowerCase() : null;

  postToN8n(N8N_MOD_WEBHOOK, {
    event: 'discord.mod_check',
    guildId: message.guildId,
    channelId: message.channelId,
    messageId: message.id,
    content: rawContent,
    commandName,
    author: {
      id: message.author.id,
      username: message.author.globalName || message.author.username,
      isMod,
    },
  }).catch(err => emitObservabilityError('discord.mod_check', err, { guildId: message.guildId }));
});

async function forwardReaction(event, reaction, user) {
  if (user?.bot) return;
  try {
    if (reaction.partial) await reaction.fetch();
    const message = reaction.message;
    if (!message?.guildId) return;
    const emojiKey = reaction.emoji.id
      ? `${reaction.emoji.name}:${reaction.emoji.id}`
      : reaction.emoji.name;
    await postToN8n(N8N_MOD_WEBHOOK, {
      event,
      guildId: message.guildId,
      channelId: message.channelId,
      messageId: message.id,
      userId: user.id,
      emoji: emojiKey,
    });
  } catch (err) {
    await emitObservabilityError(event, err);
  }
}

client.on('messageReactionAdd', (reaction, user) => void forwardReaction('discord.reaction_add', reaction, user));
client.on('messageReactionRemove', (reaction, user) => void forwardReaction('discord.reaction_remove', reaction, user));

client.on('interactionCreate', async interaction => {
  if (!interaction.isChatInputCommand()) return;

  const commandName = interaction.commandName;
  const targetWebhook = COMMUNITY_COMMANDS.has(commandName)
    ? N8N_COMMUNITY_WEBHOOK
    : MOD_COMMANDS.has(commandName)
      ? N8N_MOD_WEBHOOK
      : null;

  if (!targetWebhook) return;

  const ephemeral = EPHEMERAL_COMMANDS.has(commandName);
  try {
    await interaction.deferReply({ ephemeral });
  } catch (err) {
    await emitObservabilityError(`discord.defer.${commandName}`, err);
    return;
  }

  const options = {};
  for (const opt of interaction.options.data) {
    options[opt.name] = opt.value;
  }

  try {
    await postToN8n(targetWebhook, {
      event: 'discord.slash_command',
      commandName,
      options,
      guildId: interaction.guildId || null,
      channelId: interaction.channelId,
      interactionId: interaction.id,
      applicationId: interaction.applicationId,
      interactionToken: interaction.token,
      webhookPatchUrl: `https://discord.com/api/v10/webhooks/${interaction.applicationId}/${interaction.token}/messages/@original`,
      user: {
        id: interaction.user.id,
        username: interaction.user.username,
        globalName: interaction.user.globalName || interaction.user.username,
      },
    }, { timeoutMs: 25000 });
  } catch (err) {
    await emitObservabilityError(`discord.slash.${commandName}`, err, { guildId: interaction.guildId });
    await interaction.editReply({
      content: 'Ops, não consegui falar com o fluxo da Usami no n8n agora. Já registrei o erro na observabilidade!',
    }).catch(() => { });
  }
});

client.on('error', err => {
  console.error('[discord-bridge] Erro no cliente Discord:', redactSecrets(err?.message || err));
  void emitObservabilityError('discord.client_error', err);
});

client.login(DISCORD_BOT_TOKEN).catch(async err => {
  console.error('[discord-bridge] Falha no login do Discord:', redactSecrets(err?.message || err));
  await emitObservabilityError('discord.login_error', err);
  process.exit(1);
});
