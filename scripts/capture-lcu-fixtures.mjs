#!/usr/bin/env node
/**
 * 采集真实 LCU 响应作为测试 fixture。
 *
 * 内联最小 JSON 的单元测试发现不了"真实载荷多出一个字段就解析失败"这类问题，
 * 因此解析层的回归测试基于 `src-tauri/testdata/match/` 下的真实响应快照。
 * 本脚本用于在你自己的机器上重新采集这些快照。
 *
 * 用法（需先启动英雄联盟客户端并登录）：
 *   node scripts/capture-lcu-fixtures.mjs
 *   node scripts/capture-lcu-fixtures.mjs --beg 0 --end 19
 *   node scripts/capture-lcu-fixtures.mjs --port 12345 --token abcdef --no-redact
 *
 * 凭据来源优先级：--port/--token 参数 > lockfile > 进程命令行。
 * 部分 WeGame 渠道客户端不会写 `lockfile`（文件为 0 字节），此时退化为读取
 * `LeagueClientUx.exe` 的 `--app-port` / `--remoting-auth-token`，与 Rust 端
 * `lcu/monitor.rs` 的三重探测保持一致。
 *
 * 默认会对 puuid / summonerId / 召唤师名等身份字段做确定性脱敏（解析层不消费这些字段），
 * 避免把个人账号信息提交进仓库；需要原始数据时加 --no-redact。
 */

import fs from 'node:fs';
import fsp from 'node:fs/promises';
import https from 'node:https';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, '..');
const OUT_DIR = path.join(REPO_ROOT, 'src-tauri', 'testdata', 'match');

/** 需要脱敏的字段名（解析层不使用，替换后不影响测试语义） */
const REDACT_KEYS = new Set([
  'puuid',
  'summonerId',
  'accountId',
  'currentAccountId',
  'summonerName',
  'displayName',
  'gameName',
  'tagLine',
  'matchHistoryUri',
  'obfuscatedPuuid',
  'obfuscatedSummonerId',
]);

// ── 参数解析 ────────────────────────────────────────────────────────────────

function parseArgs(argv) {
  const opts = { beg: 0, end: 19, port: null, token: null, redact: true };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    switch (arg) {
      case '--beg':
        opts.beg = Number(argv[++i]);
        break;
      case '--end':
        opts.end = Number(argv[++i]);
        break;
      case '--port':
        opts.port = Number(argv[++i]);
        break;
      case '--token':
        opts.token = argv[++i];
        break;
      case '--no-redact':
        opts.redact = false;
        break;
      case '--help':
      case '-h':
        console.log(fs.readFileSync(fileURLToPath(import.meta.url), 'utf8').split('*/')[0]);
        process.exit(0);
        break;
      default:
        throw new Error(`未知参数: ${arg}`);
    }
  }
  if (!Number.isInteger(opts.beg) || !Number.isInteger(opts.end) || opts.end < opts.beg) {
    throw new Error('--beg / --end 需为整数且 end >= beg');
  }
  return opts;
}

// ── 凭据获取 ────────────────────────────────────────────────────────────────

/** 从 Yuumi 配置的 LolPath 反查客户端目录下的 lockfile */
function credentialsFromLockfile() {
  const appData = process.env.APPDATA;
  if (!appData) return null;

  const configPath = path.join(appData, 'Yuumi', 'config.json');
  if (!fs.existsSync(configPath)) return null;

  let cfg;
  try {
    cfg = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  } catch {
    return null;
  }

  // "WeGame" 是「启动 WeGame」的标记值，不是真实路径
  const candidates = (cfg?.General?.LolPath ?? []).filter(
    (p) => typeof p === 'string' && p !== 'WeGame',
  );

  for (const entry of candidates) {
    const dir = fs.statSync(entry, { throwIfNoEntry: false })?.isDirectory()
      ? entry
      : path.dirname(entry);
    const lockfile = path.join(dir, 'lockfile');
    if (!fs.existsSync(lockfile)) continue;

    // 格式: name:pid:port:password:protocol
    const raw = fs.readFileSync(lockfile, 'utf8').trim();
    // WeGame 渠道客户端可能写出 0 字节的 lockfile，此时字段不足，跳过继续尝试其他路径
    const parts = raw.split(':');
    if (parts.length < 5) continue;
    const port = Number(parts[2]);
    if (!Number.isInteger(port) || port <= 0) continue;
    return { port, token: parts[3], source: lockfile };
  }
  return null;
}

/** 从 LeagueClientUx.exe 的进程命令行读取 --app-port / --remoting-auth-token */
function credentialsFromProcess() {
  let stdout;
  try {
    stdout = execFileSync(
      'powershell.exe',
      [
        '-NoProfile',
        '-NonInteractive',
        '-Command',
        "(Get-CimInstance Win32_Process -Filter \"name='LeagueClientUx.exe'\").CommandLine",
      ],
      { encoding: 'utf8', windowsHide: true, timeout: 15000 },
    );
  } catch {
    return null;
  }

  const portMatch = /--app-port=(\d+)/.exec(stdout);
  const tokenMatch = /--remoting-auth-token=([^\s"]+)/.exec(stdout);
  if (!portMatch || !tokenMatch) return null;
  return {
    port: Number(portMatch[1]),
    token: tokenMatch[1],
    source: 'LeagueClientUx.exe 进程命令行',
  };
}

function resolveCredentials(opts) {
  if (opts.port && opts.token) {
    return { port: opts.port, token: opts.token, source: '命令行参数' };
  }
  const fromLockfile = credentialsFromLockfile();
  if (fromLockfile) return fromLockfile;
  const fromProcess = credentialsFromProcess();
  if (fromProcess) return fromProcess;
  throw new Error(
    '未找到 LCU 凭据。请确认客户端已启动并登录，或用 --port/--token 显式指定。',
  );
}

// ── LCU 请求 ────────────────────────────────────────────────────────────────

function lcuGet(port, token, apiPath) {
  const auth = `Basic ${Buffer.from(`riot:${token}`).toString('base64')}`;
  return new Promise((resolve, reject) => {
    const req = https.request(
      {
        host: '127.0.0.1',
        port,
        path: apiPath,
        method: 'GET',
        headers: { Authorization: auth, Accept: 'application/json' },
        rejectUnauthorized: false, // LCU 使用自签证书
        timeout: 15000,
      },
      (res) => {
        const chunks = [];
        res.on('data', (chunk) => chunks.push(chunk));
        res.on('end', () => {
          const body = Buffer.concat(chunks).toString('utf8');
          if (res.statusCode >= 400) {
            const err = new Error(`${apiPath} → HTTP ${res.statusCode}`);
            err.statusCode = res.statusCode;
            err.body = body.slice(0, 200);
            reject(err);
            return;
          }
          try {
            resolve(body ? JSON.parse(body) : null);
          } catch (e) {
            reject(new Error(`${apiPath} 返回非 JSON: ${e.message}`));
          }
        });
      },
    );
    req.on('timeout', () => req.destroy(new Error(`${apiPath} 请求超时`)));
    req.on('error', reject);
    req.end();
  });
}

// ── 脱敏 ────────────────────────────────────────────────────────────────────

function redact(node) {
  if (Array.isArray(node)) return node.map(redact);
  if (node === null || typeof node !== 'object') return node;
  const out = {};
  for (const [key, value] of Object.entries(node)) {
    if (REDACT_KEYS.has(key) && typeof value === 'string' && value.length > 0) {
      out[key] = '<redacted>';
    } else if (REDACT_KEYS.has(key) && typeof value === 'number') {
      out[key] = 0;
    } else {
      out[key] = redact(value);
    }
  }
  return out;
}

// ── 主流程 ──────────────────────────────────────────────────────────────────

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const { port, token, source } = resolveCredentials(opts);
  console.log(`LCU 凭据来源: ${source} (port=${port})`);

  await fsp.mkdir(OUT_DIR, { recursive: true });

  const summoner = await lcuGet(port, token, '/lol-summoner/v1/current-summoner').catch(
    (e) => {
      console.warn(`读取当前召唤师失败（忽略）: ${e.message}`);
      return null;
    },
  );

  const historyPath = `/lol-match-history/v1/products/lol/current-summoner/matches?begIndex=${opts.beg}&endIndex=${opts.end}`;
  const history = await lcuGet(port, token, historyPath);
  const games = history?.games?.games ?? [];
  if (games.length === 0) {
    throw new Error('对局历史为空，请先在客户端打完至少一局，或调整 --beg/--end');
  }

  const transform = opts.redact ? redact : (v) => v;
  const written = [];

  for (const game of games) {
    const fileName = `lcu-${game.queueId}-${game.gameId}.json`;
    // 与 fixture 约定一致：保留 LCU 外层响应结构，便于直接用 LcuMatchHistoryResponse 反序列化
    const wrapper = {
      games: {
        gameCount: 1,
        gameIndexBegin: 0,
        gameIndexEnd: 0,
        games: [transform(game)],
      },
    };
    await fsp.writeFile(
      path.join(OUT_DIR, fileName),
      `${JSON.stringify(wrapper, null, 2)}\n`,
      'utf8',
    );
    written.push({
      file: fileName,
      queueId: game.queueId,
      gameId: game.gameId,
      gameDuration: game.gameDuration,
      gameMode: game.gameMode,
    });
    console.log(`  ✓ ${fileName}`);
  }

  // 原始列表一并留存，便于后续补充其他解析路径的测试
  await fsp.writeFile(
    path.join(OUT_DIR, 'raw-match-history.json'),
    `${JSON.stringify(transform(history), null, 2)}\n`,
    'utf8',
  );

  // 只有在对应阶段才有的会话，取不到属正常情况
  for (const [apiPath, fileName] of [
    ['/lol-summoner/v1/current-summoner', 'current-summoner.json'],
    ['/lol-gameflow/v1/session', 'gameflow-session.json'],
    ['/lol-champ-select/v1/session', 'champ-select-session.json'],
  ]) {
    try {
      const body = await lcuGet(port, token, apiPath);
      await fsp.writeFile(
        path.join(OUT_DIR, fileName),
        `${JSON.stringify(transform(body), null, 2)}\n`,
        'utf8',
      );
      console.log(`  ✓ ${fileName}`);
    } catch (e) {
      console.log(`  - ${fileName} 跳过（${e.message}）`);
    }
  }

  const manifest = {
    schemaVersion: 1,
    capturedAt: new Date().toISOString(),
    source: {
      endpoint: historyPath,
      platformId: summoner?.platformId ?? null,
      redacted: opts.redact,
    },
    games: written,
  };
  await fsp.writeFile(
    path.join(OUT_DIR, 'manifest.json'),
    `${JSON.stringify(manifest, null, 2)}\n`,
    'utf8',
  );

  console.log(`\n完成：${written.length} 局已写入 ${path.relative(process.cwd(), OUT_DIR)}`);
  console.log('下一步：pnpm test:rust 确认解析层测试仍通过。');
}

main().catch((e) => {
  console.error(`采集失败: ${e.message}`);
  if (e.body) console.error(`响应片段: ${e.body}`);
  process.exit(1);
});
