#!/usr/bin/env node
/**
 * Usami QA Review - roda no self-hosted runner do notebook Linux
 * Usa BRAIN_MODEL (NVIDIA nemotron-3-ultra) via API NVIDIA direta para code review
 * Posta comentário na PR como Usamiin via gh CLI (token no keychain)
 */

import { spawn } from 'node:child_process';

const PR_NUMBER = process.argv[2];
const REPO = 'UsamiVtuber/usami-workspace';
const MAX_DIFF_CHARS = 120000;

if (!PR_NUMBER) {
  console.error('Uso: node scripts/qa-review.mjs <PR_NUMBER>');
  process.exit(1);
}

async function runGh(args) {
  return new Promise((resolve, reject) => {
    const proc = spawn('gh', args, { stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    proc.stdout.on('data', d => stdout += d);
    proc.stderr.on('data', d => stderr += d);
    proc.on('close', code => {
      if (code === 0) resolve(stdout.trim());
      else reject(new Error(`gh ${args.join(' ')} falhou (${code}): ${stderr}`));
    });
  });
}

async function getDiff() {
  const diff = await runGh(['pr', 'diff', PR_NUMBER, '--repo', REPO]);
  if (diff.length <= MAX_DIFF_CHARS) return diff;
  return `${diff.slice(0, MAX_DIFF_CHARS)}\n\n[diff truncado em ${MAX_DIFF_CHARS} caracteres — revise o restante no GitHub]`;
}

async function getPRInfo() {
  const json = await runGh(['pr', 'view', PR_NUMBER, '--repo', REPO, '--json', 'title,author,baseRefName,headRefName,files']);
  return JSON.parse(json);
}

async function postComment(body) {
  await runGh(['pr', 'comment', PR_NUMBER, '--repo', REPO, '--body', body]);
  console.log('✅ Comentário postado na PR #' + PR_NUMBER);
}

function buildReviewPrompt(diff, prInfo) {
  return `Você é a **Usami**, companheira virtual e agente de QA do workspace Usami.
Sua tarefa: fazer **code review completo** da PR #${PR_NUMBER} e emitir veredito.

## Contexto da PR
- Título: ${prInfo.title}
- Autor: ${prInfo.author.login}
- Base: ${prInfo.baseRefName} → Head: ${prInfo.headRefName}
- Arquivos alterados: ${prInfo.files.map(f => f.path).join(', ')}

## Regras de review (AGENTS.md da raiz)
1. **Segurança e privacidade**: nunca vazar segredos (\`.env\`, chaves, tokens, dados pessoais); \`vault/\` é privado — leitura em lote ou alteração fora das pastas de automação documentadas é bloqueio.
2. **Operações locais**: não abrir portas, publicar serviços, alterar túneis nem remover volumes/backups Docker sem autorização explícita.
3. **Menor mudança**: diff focado no componente certo (\`usami/\`, \`usami-desktop/\`, \`infra/n8n-local/\`, \`usami-monitor/\`), sem refatorações fora do escopo.
4. **Componente**: \`usami/\` segue \`usami/AGENTS.md\`; \`usami-desktop/\` não pode regredir erros de \`usami-desktop/docs/KNOWN_ERRORS.md\`; \`infra/n8n-local/\` segue os READMEs e o export não pode divergir da instância ativa sem nota.
5. **Testes**: não desativar testes para mascarar falhas; exigir as verificações coerentes com o componente (\`npm test\`/\`npm run build\` em \`usami/\`, docs e execução real nos workflows do n8n).
6. **Contratos e docs**: endpoints e payloads alinhados ao código real; documentação não pode afirmar comportamento não provado.

## Formato da resposta (JSON obrigatório)
{
  "veredito": "APROVADO" | "PRECISA_AJUSTES",
  "resumo": "2-3 frases do que foi avaliado",
  "checks": {
    "testes": "PASS" | "FAIL" | "NA",
    "seguranca": "OK" | "ALERTA" | "BLOQUEIO",
    "arquitetura": "OK" | "ALERTA" | "BLOQUEIO",
    "observabilidade": "OK" | "ALERTA"
  },
  "comentarios": [
    { "arquivo": "caminho/arquivo.ext", "linha": 123, "severidade": "info|warn|blocker", "mensagem": "..." }
  ],
  "sugestoes": [
    "Sugestão opcional de melhoria não-bloqueante"
  ]
}

## Diff da PR
\`\`\`diff
${diff}
\`\`\`

Responda **apenas** com o JSON acima.`;
}

async function callBrainModel(prompt) {
  const apiKey = process.env.NVIDIA_API_KEY;
  if (!apiKey) throw new Error('NVIDIA_API_KEY não definido (organization secret)');

  const model = process.env.BRAIN_MODEL || 'nvidia/nemotron-3-ultra-550b-a55b';

  const res = await fetch('https://integrate.api.nvidia.com/v1/chat/completions', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${apiKey}`,
      'Content-Type': 'application/json',
      'Accept': 'application/json'
    },
    body: JSON.stringify({
      model,
      messages: [{ role: 'user', content: prompt }],
      response_format: { type: 'json_object' },
      temperature: 0.2,
      max_tokens: 3000
    })
  });

  if (!res.ok) {
    const err = await res.text();
    throw new Error(`NVIDIA API ${res.status}: ${err}`);
  }

  const data = await res.json();
  const content = data.choices?.[0]?.message?.content;
  if (!content) throw new Error('Resposta vazia do modelo');
  return JSON.parse(content);
}

function formatComment(review) {
  const badge = review.veredito === 'APROVADO' ? '✅ **APROVADO**' : '❌ **PRECISA AJUSTES**';
  const lines = [
    `## ${badge} — QA Review pela Usami`,
    '',
    review.resumo,
    '',
    '### Checks',
    `| Check | Status |`,
    `|-------|--------|`,
    `| Testes | ${review.checks.testes} |`,
    `| Segurança | ${review.checks.seguranca} |`,
    `| Arquitetura | ${review.checks.arquitetura} |`,
    `| Observabilidade | ${review.checks.observabilidade} |`,
    ''
  ];

  if (review.comentarios.length > 0) {
    lines.push('### Comentários por arquivo');
    lines.push('');
    for (const c of review.comentarios) {
      const icon = c.severidade === 'blocker' ? '🚨' : c.severidade === 'warn' ? '⚠️' : 'ℹ️';
      lines.push(`${icon} **${c.arquivo}:${c.linha}** — ${c.mensagem}`);
    }
    lines.push('');
  }

  if (review.sugestoes.length > 0) {
    lines.push('### Sugestões (não-bloqueantes)');
    lines.push('');
    for (const s of review.sugestoes) lines.push(`- ${s}`);
    lines.push('');
  }

  lines.push('---');
  lines.push('*Review gerado por Usami via `BRAIN_MODEL` (NVIDIA Nemotron 3 Ultra). Veredito final é do mantenedor.*');

  return lines.join('\n');
}

async function main() {
  console.log(`🔍 Iniciando QA Review da PR #${PR_NUMBER}...`);

  try {
    const [diff, prInfo] = await Promise.all([getDiff(), getPRInfo()]);
    console.log('📄 Diff obtido, chamando modelo...');

    const prompt = buildReviewPrompt(diff, prInfo);
    const review = await callBrainModel(prompt);
    console.log(`📋 Veredito: ${review.veredito}`);

    const comment = formatComment(review);
    await postComment(comment);

    if (review.veredito === 'PRECISA_AJUSTES') {
      console.log('⚠️ PR precisa de ajustes — ver comentários acima');
      process.exit(1); // sinaliza falha no workflow
    } else {
      console.log('🎉 PR aprovado pela Usami!');
      process.exit(0);
    }
  } catch (err) {
    console.error('❌ Erro no QA Review:', err.message);
    // Posta erro como comentário para visibilidade
    await postComment(`## ❌ Erro no QA Review automatizado\n\n\`\`\`\n${err.message}\n\`\`\`\n\n*Execute manualmente ou verifique logs do runner.*`).catch(() => {});
    process.exit(2);
  }
}

main();
