import { promises as fs } from 'node:fs';
import path from 'node:path';
import readline from 'node:readline/promises';
import { stdin as input, stdout as output } from 'node:process';
import { findConfiguredVault, getConfiguredVaults, readConfig, savePairedVault, markVaultSynced } from './lib/config.mjs';
import { discoverRegisteredVaults, resolveManualVault, scanForVaults } from './lib/obsidian.mjs';
import { readVaultManifest, searchManifest, writeVaultManifest } from './lib/manifest.mjs';
import { buildObsidianOpenUri, openExternalUri } from './lib/open-uri.mjs';

const HELP = `Aleph Obsidian bridge

Usage:
  aleph obsidian pair [path] [--scan]
  aleph obsidian vaults
  aleph obsidian status [--vault <name|id|path>]
  aleph obsidian sync [--vault <name|id|path>]
  aleph obsidian search <query> [--vault <name|id|path>] [--limit 10]
  aleph obsidian capture <text> [--title <title>] [--folder <folder>] [--vault <name|id|path>]
  aleph obsidian open [note-path] [--vault <name|id|path>] [--print]

Aliases:
  aleph pair      Same as aleph obsidian pair
  aleph vaults    Same as aleph obsidian vaults
`;

export async function run(argv, io = { input, output, console }) {
  const [rawCommand, ...rest] = argv;

  if (!rawCommand || rawCommand === '--help' || rawCommand === '-h') {
    io.console.log(HELP.trim());
    return;
  }

  if (rawCommand === 'pair') {
    await handlePair(parseArgs(rest), io);
    return;
  }

  if (rawCommand === 'vaults') {
    await handleVaults(io);
    return;
  }

  if (rawCommand !== 'obsidian') {
    throw new Error(`Unknown command: ${rawCommand}\n\n${HELP.trim()}`);
  }

  const [subcommand, ...subcommandArgs] = rest;
  const args = parseArgs(subcommandArgs);

  switch (subcommand) {
    case 'pair':
      await handlePair(args, io);
      break;
    case 'vaults':
    case 'list':
      await handleVaults(io);
      break;
    case 'status':
      await handleStatus(args, io);
      break;
    case 'sync':
      await handleSync(args, io);
      break;
    case 'search':
      await handleSearch(args, io);
      break;
    case 'capture':
      await handleCapture(args, io);
      break;
    case 'open':
      await handleOpen(args, io);
      break;
    default:
      throw new Error(`Unknown Obsidian command: ${subcommand || '(missing)'}\n\n${HELP.trim()}`);
  }
}

function parseArgs(argv) {
  const flags = {};
  const positionals = [];

  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];

    if (!token.startsWith('--')) {
      positionals.push(token);
      continue;
    }

    const [rawKey, inlineValue] = token.slice(2).split('=', 2);
    const key = rawKey.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());

    if (inlineValue !== undefined) {
      flags[key] = inlineValue;
      continue;
    }

    const next = argv[index + 1];
    if (next && !next.startsWith('--')) {
      flags[key] = next;
      index += 1;
    } else {
      flags[key] = true;
    }
  }

  return { flags, positionals };
}

async function handlePair(args, io) {
  let vault;
  const [manualPath] = args.positionals;

  if (manualPath) {
    vault = await resolveManualVault(manualPath);
  } else {
    const registeredVaults = await discoverRegisteredVaults({
      onWarning: (warning) => io.console.warn(warning),
    });
    const scannedVaults = args.flags.scan ? await scanForVaults() : [];
    const candidates = dedupeByPath([...registeredVaults, ...scannedVaults]);

    if (candidates.length === 0) {
      throw new Error('No Obsidian vaults found. Re-run with a vault path, or use --scan to search common folders.');
    }

    vault = await pickVault(candidates, io);
  }

  const { configPath, vault: savedVault } = await savePairedVault(vault);
  const { manifest, manifestPath } = await writeVaultManifest(savedVault);

  await markVaultSynced(savedVault);

  io.console.log(`Paired ${savedVault.name}`);
  io.console.log(`Vault: ${savedVault.path}`);
  io.console.log(`Config: ${configPath}`);
  io.console.log(`Manifest: ${manifestPath}`);
  io.console.log(`Indexed ${manifest.summary.notes} notes, ${manifest.summary.tags} tags, ${manifest.summary.openTasks} open tasks.`);
}

async function handleVaults(io) {
  const config = await readConfig();
  const vaults = getConfiguredVaults(config);

  if (vaults.length === 0) {
    io.console.log('No paired vaults yet. Run: aleph obsidian pair');
    return;
  }

  for (const vault of vaults) {
    const activeMarker = vault.id === config.activeVaultId ? '*' : ' ';
    io.console.log(`${activeMarker} ${vault.name} (${vault.id})`);
    io.console.log(`  ${vault.path}`);
    io.console.log(`  synced: ${vault.lastSyncedAt || 'never'}`);
  }
}

async function handleStatus(args, io) {
  const vault = await requireVault(args.flags.vault);
  const manifest = await readVaultManifest(vault).catch(() => null);

  io.console.log(`${vault.name}`);
  io.console.log(`Path: ${vault.path}`);
  io.console.log(`Source: ${vault.source}`);
  io.console.log(`Last synced: ${vault.lastSyncedAt || 'never'}`);

  if (!manifest) {
    io.console.log('Manifest: missing. Run: aleph obsidian sync');
    return;
  }

  io.console.log(`Manifest: ${manifest.generatedAt}`);
  io.console.log(`Notes: ${manifest.summary.notes}`);
  io.console.log(`Tags: ${manifest.summary.tags}`);
  io.console.log(`Open tasks: ${manifest.summary.openTasks}`);
  io.console.log(`Words: ${manifest.summary.words}`);
}

async function handleSync(args, io) {
  const vault = await requireVault(args.flags.vault);
  const { manifest, manifestPath } = await writeVaultManifest(vault);
  await markVaultSynced(vault);

  io.console.log(`Synced ${vault.name}`);
  io.console.log(`Manifest: ${manifestPath}`);
  io.console.log(`Indexed ${manifest.summary.notes} notes, ${manifest.summary.tags} tags, ${manifest.summary.openTasks} open tasks.`);
}

async function handleSearch(args, io) {
  const query = args.positionals.join(' ').trim();
  if (!query) {
    throw new Error('Search query is required.');
  }

  const vault = await requireVault(args.flags.vault);
  const manifest = await getManifestOrSync(vault);
  const results = searchManifest(manifest, query, { limit: Number(args.flags.limit) || 10 });

  if (results.length === 0) {
    io.console.log(`No matches for "${query}" in ${vault.name}.`);
    return;
  }

  results.forEach(({ note, score }, index) => {
    const tags = note.tags?.length ? ` #${note.tags.join(' #')}` : '';
    io.console.log(`${index + 1}. ${note.path} (${score})${tags}`);
  });
}

async function handleCapture(args, io) {
  const body = args.positionals.join(' ').trim();
  if (!body) {
    throw new Error('Capture text is required.');
  }

  const vault = await requireVault(args.flags.vault);
  const title = args.flags.title || makeCaptureTitle(body);
  const folder = args.flags.folder || 'Aleph Inbox';
  let relativePath = safeVaultRelativePath(folder, `${slugify(title)}.md`);
  let absolutePath = path.join(vault.path, relativePath);

  await fs.mkdir(path.dirname(absolutePath), { recursive: true });

  try {
    await fs.writeFile(absolutePath, renderCapture(title, body), { flag: 'wx' });
  } catch (error) {
    if (error?.code !== 'EEXIST') {
      throw error;
    }

    relativePath = safeVaultRelativePath(folder, `${slugify(title)}-${Date.now()}.md`);
    absolutePath = path.join(vault.path, relativePath);
    await fs.writeFile(absolutePath, renderCapture(title, body), 'utf8');
  }

  await handleSync({ flags: { vault: vault.id }, positionals: [] }, { ...io, console: quietConsole(io.console) });
  io.console.log(`Captured: ${relativePath}`);
}

async function handleOpen(args, io) {
  const vault = await requireVault(args.flags.vault);
  const notePath = args.positionals.join(' ').trim();
  const uri = buildObsidianOpenUri(vault, notePath || undefined);

  if (args.flags.print) {
    io.console.log(uri);
    return;
  }

  await openExternalUri(uri);
  io.console.log(`Opening ${notePath || vault.name} in Obsidian.`);
}

async function requireVault(selector) {
  const config = await readConfig();
  const vault = findConfiguredVault(config, selector);

  if (!vault) {
    throw new Error('No paired vault found. Run: aleph obsidian pair');
  }

  return vault;
}

async function getManifestOrSync(vault) {
  try {
    return await readVaultManifest(vault);
  } catch (error) {
    if (error?.code !== 'ENOENT') {
      throw error;
    }

    const { manifest } = await writeVaultManifest(vault);
    await markVaultSynced(vault);
    return manifest;
  }
}

async function pickVault(vaults, io) {
  if (vaults.length === 1 || !io.input.isTTY) {
    return vaults[0];
  }

  io.console.log('Pick an Obsidian vault to pair with Aleph:');
  vaults.forEach((vault, index) => {
    io.console.log(`${index + 1}. ${vault.name}`);
    io.console.log(`   ${vault.path}`);
  });

  const reader = readline.createInterface({ input: io.input, output: io.output });

  try {
    const answer = await reader.question('Vault number: ');
    const selectedIndex = Number(answer) - 1;

    if (!Number.isInteger(selectedIndex) || !vaults[selectedIndex]) {
      throw new Error('Invalid vault selection.');
    }

    return vaults[selectedIndex];
  } finally {
    reader.close();
  }
}

function dedupeByPath(vaults) {
  return [...new Map(vaults.map((vault) => [path.resolve(vault.path), vault])).values()];
}

function makeCaptureTitle(body) {
  return body.split('\n')[0].slice(0, 64).trim() || 'Aleph capture';
}

function slugify(value) {
  const slug = value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 80);

  return slug || `capture-${Date.now()}`;
}

function safeVaultRelativePath(folder, fileName) {
  const relativePath = path.join(folder, fileName);
  const normalized = path.normalize(relativePath);

  if (path.isAbsolute(normalized) || normalized.startsWith('..')) {
    throw new Error('Capture path must stay inside the selected vault.');
  }

  return normalized;
}

function renderCapture(title, body) {
  const now = new Date().toISOString();

  return `---\ntitle: "${title.replaceAll('"', '\\"')}"\ncreated: ${now}\nsource: aleph\ntags: [aleph/capture]\n---\n\n# ${title}\n\n${body}\n`;
}

function quietConsole(parentConsole) {
  return {
    ...parentConsole,
    log: () => {},
  };
}
