#!/usr/bin/env node
/**
 * Prepara assets do Discord Rich Presence a partir das expressões do avatar.
 *
 * Uso: node scripts/prepare-discord-assets.mjs
 *
 * Gera em: assets/discord-rp-assets/
 *   - large/ (512x512)
 *   - small/ (256x256)
 *
 * Requer: sharp (já disponível via @xenova/transformers)
 */

import fs from 'node:fs/promises';
import path from 'node:path';
import sharp from 'sharp';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, '..');
const SOURCE_DIR = path.join(PROJECT_ROOT, 'assets', 'images', 'expressions');
const OUTPUT_DIR = path.join(PROJECT_ROOT, 'usami', 'assets', 'discord-rp-assets');

const LARGE_SIZE = 512;
const SMALL_SIZE = 256;

const MAPPING = {
  // Large images
  'usami': { src: '00-baseline.png', size: LARGE_SIZE, dir: 'large' },
  'usami_idle': { src: '00-baseline.png', size: LARGE_SIZE, dir: 'large' },
  'usami_chat': { src: '01-prefab_happy01.png', size: LARGE_SIZE, dir: 'large' },
  'usami_thinking': { src: '19-prefab_doubt.png', size: LARGE_SIZE, dir: 'large' },
  'usami_sleep': { src: '12-prefab_sad01.png', size: LARGE_SIZE, dir: 'large' },
  'usami_tool': { src: '21-prefab_emoji.png', size: LARGE_SIZE, dir: 'large' },

  // Small images
  'happy': { src: '01-prefab_happy01.png', size: SMALL_SIZE, dir: 'small' },
  'tsundere': { src: '07-prefab_angry01.png', size: SMALL_SIZE, dir: 'small' },
  'surprised': { src: '20-prefab_startled.png', size: SMALL_SIZE, dir: 'small' },
  'thinking': { src: '19-prefab_doubt.png', size: SMALL_SIZE, dir: 'small' },
  'neutral': { src: '00-baseline.png', size: SMALL_SIZE, dir: 'small' },
  'sad': { src: '12-prefab_sad01.png', size: SMALL_SIZE, dir: 'small' },
  'angry': { src: '07-prefab_angry01.png', size: SMALL_SIZE, dir: 'small' },
  'sleep': { src: '12-prefab_sad01.png', size: SMALL_SIZE, dir: 'small' },
  'tool': { src: '21-prefab_emoji.png', size: SMALL_SIZE, dir: 'small' },
};

async function ensureDir(dir) {
  await fs.mkdir(dir, { recursive: true });
}

async function processAsset(key, config) {
  const srcPath = path.join(SOURCE_DIR, config.src);
  const outDir = path.join(OUTPUT_DIR, config.dir);
  const outPath = path.join(outDir, `${key}.png`);

  try {
    await ensureDir(outDir);

    // Verifica se arquivo fonte existe
    await fs.access(srcPath);

    // Processa: redimensiona mantendo aspect ratio, preenche com transparência se necessário
    await sharp(srcPath)
      .resize(config.size, config.size, {
        fit: 'contain',
        background: { r: 0, g: 0, b: 0, alpha: 0 },
      })
      .png({ compressionLevel: 9 })
      .toFile(outPath);

    const stats = await fs.stat(outPath);
    console.log(`✅ ${key}: ${config.src} → ${outPath} (${(stats.size / 1024).toFixed(1)} KB)`);
    return true;
  } catch (err) {
    console.error(`❌ ${key}: ${err.message}`);
    return false;
  }
}

async function main() {
  console.log('🎨 Preparando assets do Discord Rich Presence...\n');
  console.log(`📂 Origem: ${SOURCE_DIR}`);
  console.log(`📂 Destino: ${OUTPUT_DIR}`);
  console.log(`📐 Large: ${LARGE_SIZE}x${LARGE_SIZE} | Small: ${SMALL_SIZE}x${SMALL_SIZE}\n`);

  // Limpa diretório de saída anterior
  try {
    await fs.rm(OUTPUT_DIR, { recursive: true, force: true });
  } catch {}

  let success = 0;
  let failed = 0;

  for (const [key, config] of Object.entries(MAPPING)) {
    const ok = await processAsset(key, config);
    if (ok) success++;
    else failed++;
  }

  console.log(`\n📊 Resultado: ${success} sucessos, ${failed} falhas`);

  if (failed === 0) {
    console.log('\n✨ Pronto! Faça upload dos arquivos em:');
    console.log(`   ${path.resolve(OUTPUT_DIR, 'large')}/`);
    console.log(`   ${path.resolve(OUTPUT_DIR, 'small')}/`);
    console.log('\nNo Discord Developer Portal:');
    console.log('   Applications → 1571656265107771392 → Rich Presence → Art Assets');
  }
}

main().catch(console.error);