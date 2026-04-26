import { promises as fs } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { normalizeVaultRecord, vaultFingerprint } from './config.mjs';

const IGNORED_DIRS = new Set(['.git', '.hg', '.svn', '.obsidian', '.trash', 'node_modules']);
const NOTE_EXTENSIONS = new Set(['.md', '.canvas']);

export function expandHome(value, home = os.homedir()) {
  if (!value) {
    return value;
  }

  if (value === '~') {
    return home;
  }

  if (value.startsWith('~/') || value.startsWith('~\\')) {
    return path.join(home, value.slice(2));
  }

  return value;
}

export async function pathExists(candidate) {
  try {
    await fs.access(candidate);
    return true;
  } catch {
    return false;
  }
}

export function getObsidianConfigCandidates(env = process.env, platform = process.platform) {
  const candidates = [];

  if (env.OBSIDIAN_CONFIG) {
    candidates.push(path.resolve(expandHome(env.OBSIDIAN_CONFIG)));
  }

  if (platform === 'win32') {
    const appData = env.APPDATA || path.join(os.homedir(), 'AppData', 'Roaming');
    candidates.push(
      path.join(appData, 'obsidian', 'obsidian.json'),
      path.join(appData, 'Obsidian', 'obsidian.json'),
    );
  } else if (platform === 'darwin') {
    candidates.push(
      path.join(os.homedir(), 'Library', 'Application Support', 'obsidian', 'obsidian.json'),
      path.join(os.homedir(), 'Library', 'Application Support', 'Obsidian', 'obsidian.json'),
    );
  } else {
    const xdgConfig = env.XDG_CONFIG_HOME || path.join(os.homedir(), '.config');
    candidates.push(
      path.join(xdgConfig, 'obsidian', 'obsidian.json'),
      path.join(xdgConfig, 'Obsidian', 'obsidian.json'),
    );
  }

  return [...new Set(candidates)];
}

export async function readObsidianConfig(configPath) {
  const raw = await fs.readFile(configPath, 'utf8');
  return JSON.parse(raw);
}

export function vaultsFromObsidianConfig(config, sourcePath) {
  const vaultEntries = Object.entries(config?.vaults || {});

  return vaultEntries
    .map(([obsidianId, value]) => {
      const vaultPath = value?.path || value?.uri || value?.folder;

      if (!vaultPath) {
        return null;
      }

      const expandedPath = path.resolve(expandHome(vaultPath));

      return normalizeVaultRecord({
        id: obsidianId || vaultFingerprint(expandedPath),
        obsidianId,
        name: value?.name || path.basename(expandedPath),
        path: expandedPath,
        source: sourcePath,
      });
    })
    .filter(Boolean);
}

export async function discoverRegisteredVaults(options = {}) {
  const env = options.env || process.env;
  const platform = options.platform || process.platform;
  const candidates = options.configPaths || getObsidianConfigCandidates(env, platform);
  const vaults = [];

  for (const configPath of candidates) {
    try {
      const config = await readObsidianConfig(configPath);
      vaults.push(...vaultsFromObsidianConfig(config, configPath));
    } catch (error) {
      if (error?.code !== 'ENOENT') {
        options.onWarning?.(`Skipped ${configPath}: ${error.message}`);
      }
    }
  }

  return dedupeVaults(await filterExistingVaults(vaults));
}

export async function filterExistingVaults(vaults) {
  const checked = await Promise.all(vaults.map(async (vault) => {
    return (await isVaultPath(vault.path)) ? vault : null;
  }));

  return checked.filter(Boolean);
}

export async function isVaultPath(candidate) {
  if (!candidate) {
    return false;
  }

  const stats = await fs.stat(candidate).catch(() => null);
  if (!stats?.isDirectory()) {
    return false;
  }

  return pathExists(path.join(candidate, '.obsidian'));
}

export function dedupeVaults(vaults) {
  const byPath = new Map();

  for (const vault of vaults) {
    byPath.set(path.resolve(vault.path), vault);
  }

  return [...byPath.values()].sort((left, right) => left.name.localeCompare(right.name));
}

export function defaultScanRoots(env = process.env, platform = process.platform) {
  const home = os.homedir();
  const roots = [
    path.join(home, 'Documents'),
    path.join(home, 'Desktop'),
    path.join(home, 'Notes'),
    path.join(home, 'Obsidian'),
    path.join(home, 'Vaults'),
  ];

  if (platform === 'darwin') {
    roots.push(
      path.join(home, 'Library', 'Mobile Documents', 'iCloud~md~obsidian', 'Documents'),
      path.join(home, 'Library', 'CloudStorage'),
    );
  }

  if (env.ONEDRIVE) {
    roots.push(env.ONEDRIVE);
  }

  if (env.DROPBOX) {
    roots.push(env.DROPBOX);
  }

  return [...new Set(roots.map((root) => path.resolve(root)))];
}

export async function scanForVaults(roots = defaultScanRoots(), options = {}) {
  const maxDepth = Number.isInteger(options.maxDepth) ? options.maxDepth : 4;
  const found = [];

  async function visit(directory, depth) {
    if (depth > maxDepth) {
      return;
    }

    const entries = await fs.readdir(directory, { withFileTypes: true }).catch(() => []);

    if (entries.some((entry) => entry.isDirectory() && entry.name === '.obsidian')) {
      found.push(normalizeVaultRecord({
        path: directory,
        source: 'scan',
      }));
      return;
    }

    await Promise.all(entries
      .filter((entry) => entry.isDirectory() && !IGNORED_DIRS.has(entry.name) && !entry.name.startsWith('.'))
      .map((entry) => visit(path.join(directory, entry.name), depth + 1)));
  }

  await Promise.all(roots.map((root) => visit(path.resolve(expandHome(root)), 0)));
  return dedupeVaults(found);
}

export async function resolveManualVault(vaultPath) {
  const resolvedPath = path.resolve(expandHome(vaultPath));

  if (!(await isVaultPath(resolvedPath))) {
    throw new Error(`${resolvedPath} is not an Obsidian vault. Expected a directory containing .obsidian/.`);
  }

  return normalizeVaultRecord({
    path: resolvedPath,
    source: 'manual',
  });
}

export async function readVaultNotes(vaultPath) {
  const root = path.resolve(expandHome(vaultPath));
  const notes = [];

  async function walk(directory) {
    const entries = await fs.readdir(directory, { withFileTypes: true });

    for (const entry of entries) {
      const fullPath = path.join(directory, entry.name);

      if (entry.isDirectory()) {
        if (!IGNORED_DIRS.has(entry.name)) {
          await walk(fullPath);
        }
        continue;
      }

      if (!entry.isFile() || !NOTE_EXTENSIONS.has(path.extname(entry.name).toLowerCase())) {
        continue;
      }

      const content = await fs.readFile(fullPath, 'utf8').catch(() => '');
      const relativePath = path.relative(root, fullPath).split(path.sep).join('/');
      notes.push({
        absolutePath: fullPath,
        relativePath,
        title: path.basename(entry.name, path.extname(entry.name)),
        extension: path.extname(entry.name).toLowerCase(),
        content,
        ...extractNoteMetadata(content),
      });
    }
  }

  await walk(root);
  return notes.sort((left, right) => left.relativePath.localeCompare(right.relativePath));
}

export function extractNoteMetadata(content) {
  const frontmatter = {};
  const tags = new Set();
  const links = new Set();
  const tasks = [];
  let body = content;

  if (content.startsWith('---\n')) {
    const end = content.indexOf('\n---', 4);
    if (end !== -1) {
      const rawFrontmatter = content.slice(4, end).trim();
      body = content.slice(end + 4);
      Object.assign(frontmatter, parseSimpleFrontmatter(rawFrontmatter));
    }
  }

  for (const [key, value] of Object.entries(frontmatter)) {
    const values = Array.isArray(value) ? value : [value];
    for (const item of values) {
      if (typeof item !== 'string') {
        continue;
      }

      if (key === 'tag' || key === 'tags' || item.startsWith('#')) {
        tags.add(item.replace(/^#/, ''));
      }
    }
  }

  for (const match of body.matchAll(/(^|\s)#([\p{L}\p{N}_/-]+)/gu)) {
    tags.add(match[2]);
  }

  for (const match of body.matchAll(/!?(?:\[\[([^\]|#]+)(?:[#|][^\]]*)?\]\])/g)) {
    links.add(match[1].trim());
  }

  for (const match of body.matchAll(/\[[^\]]+\]\(([^)]+\.md(?:#[^)]+)?)\)/g)) {
    links.add(decodeURIComponent(match[1].split('#')[0]).replace(/\.md$/i, ''));
  }

  body.split('\n').forEach((line, index) => {
    const task = line.match(/^\s*[-*]\s+\[([ xX])]\s+(.+)$/);
    if (task) {
      tasks.push({
        line: index + 1,
        done: task[1].toLowerCase() === 'x',
        text: task[2].trim(),
      });
    }
  });

  return {
    frontmatter,
    tags: [...tags].sort(),
    links: [...links].sort(),
    tasks,
    wordCount: countWords(body),
  };
}

export function parseSimpleFrontmatter(rawFrontmatter) {
  const result = {};
  let currentListKey = null;

  for (const line of rawFrontmatter.split('\n')) {
    const listItem = line.match(/^\s*-\s+(.+)$/);
    if (listItem && currentListKey) {
      result[currentListKey].push(cleanYamlValue(listItem[1]));
      continue;
    }

    const pair = line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/);
    if (!pair) {
      currentListKey = null;
      continue;
    }

    const [, key, value] = pair;

    if (!value) {
      result[key] = [];
      currentListKey = key;
      continue;
    }

    currentListKey = null;
    result[key] = parseYamlScalarOrList(value);
  }

  return result;
}

function parseYamlScalarOrList(value) {
  const trimmed = value.trim();

  if (trimmed.startsWith('[') && trimmed.endsWith(']')) {
    return trimmed.slice(1, -1).split(',').map((item) => cleanYamlValue(item.trim())).filter(Boolean);
  }

  return cleanYamlValue(trimmed);
}

function cleanYamlValue(value) {
  return String(value).replace(/^['\"]|['\"]$/g, '').trim();
}

function countWords(content) {
  return content.trim().split(/\s+/).filter(Boolean).length;
}
