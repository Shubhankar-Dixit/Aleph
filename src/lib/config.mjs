import { createHash } from 'node:crypto';
import { promises as fs } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export function getAlephHome(env = process.env, platform = process.platform) {
  if (env.ALEPH_HOME) {
    return path.resolve(env.ALEPH_HOME);
  }

  if (env.XDG_CONFIG_HOME) {
    return path.join(env.XDG_CONFIG_HOME, 'aleph');
  }

  if (platform === 'win32' && env.APPDATA) {
    return path.join(env.APPDATA, 'Aleph');
  }

  if (platform === 'darwin') {
    return path.join(os.homedir(), 'Library', 'Application Support', 'Aleph');
  }

  return path.join(os.homedir(), '.config', 'aleph');
}

export function getConfigPath(env = process.env, platform = process.platform) {
  if (env.ALEPH_CONFIG) {
    return path.resolve(env.ALEPH_CONFIG);
  }

  return path.join(getAlephHome(env, platform), 'obsidian.json');
}

export function emptyConfig() {
  return {
    version: 1,
    activeVaultId: null,
    vaults: {},
  };
}

export async function readConfig(env = process.env, platform = process.platform) {
  const configPath = getConfigPath(env, platform);

  try {
    const raw = await fs.readFile(configPath, 'utf8');
    const config = JSON.parse(raw);

    return {
      ...emptyConfig(),
      ...config,
      vaults: config.vaults ?? {},
    };
  } catch (error) {
    if (error?.code === 'ENOENT') {
      return emptyConfig();
    }

    throw new Error(`Unable to read Aleph config at ${configPath}: ${error.message}`);
  }
}

export async function writeConfig(config, env = process.env, platform = process.platform) {
  const configPath = getConfigPath(env, platform);
  await fs.mkdir(path.dirname(configPath), { recursive: true });
  await fs.writeFile(configPath, `${JSON.stringify(config, null, 2)}\n`, 'utf8');
  return configPath;
}

export function vaultFingerprint(vaultPath) {
  return createHash('sha1').update(path.resolve(vaultPath)).digest('hex').slice(0, 16);
}

export function normalizeVaultRecord(candidate, now = new Date()) {
  const resolvedPath = path.resolve(candidate.path);
  const id = candidate.id || candidate.obsidianId || vaultFingerprint(resolvedPath);

  return {
    id,
    name: candidate.name || path.basename(resolvedPath),
    path: resolvedPath,
    obsidianId: candidate.obsidianId || null,
    source: candidate.source || 'manual',
    pairedAt: candidate.pairedAt || now.toISOString(),
    lastSyncedAt: candidate.lastSyncedAt || null,
  };
}

export async function savePairedVault(candidate, env = process.env, platform = process.platform) {
  const config = await readConfig(env, platform);
  const vault = normalizeVaultRecord(candidate);
  config.activeVaultId = vault.id;
  config.vaults[vault.id] = {
    ...(config.vaults[vault.id] || {}),
    ...vault,
  };
  const configPath = await writeConfig(config, env, platform);

  return { config, configPath, vault: config.vaults[vault.id] };
}

export async function markVaultSynced(vault, env = process.env, platform = process.platform, now = new Date()) {
  const config = await readConfig(env, platform);
  const existing = config.vaults[vault.id] || normalizeVaultRecord(vault, now);
  config.vaults[vault.id] = {
    ...existing,
    ...vault,
    lastSyncedAt: now.toISOString(),
  };
  config.activeVaultId = vault.id;
  await writeConfig(config, env, platform);
}

export function getConfiguredVaults(config) {
  return Object.values(config.vaults || {});
}

export function findConfiguredVault(config, selector) {
  const vaults = getConfiguredVaults(config);

  if (!selector) {
    return config.activeVaultId ? config.vaults[config.activeVaultId] : vaults[0];
  }

  const normalizedSelector = selector.toLowerCase();
  const absoluteSelector = path.isAbsolute(selector) ? path.resolve(selector) : null;

  return vaults.find((vault) => {
    return vault.id === selector
      || vault.obsidianId === selector
      || vault.name.toLowerCase() === normalizedSelector
      || (absoluteSelector && path.resolve(vault.path) === absoluteSelector);
  });
}
