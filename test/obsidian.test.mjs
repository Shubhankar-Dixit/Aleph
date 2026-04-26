import assert from 'node:assert/strict';
import { promises as fs } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { savePairedVault } from '../src/lib/config.mjs';
import { buildVaultManifest, searchManifest } from '../src/lib/manifest.mjs';
import { buildObsidianOpenUri } from '../src/lib/open-uri.mjs';
import { resolveManualVault, vaultsFromObsidianConfig } from '../src/lib/obsidian.mjs';

test('reads vaults from Obsidian config shape', () => {
  const vaults = vaultsFromObsidianConfig({
    vaults: {
      alpha: {
        path: '/tmp/Alpha Vault',
        name: 'Alpha',
      },
    },
  }, '/tmp/obsidian.json');

  assert.equal(vaults.length, 1);
  assert.equal(vaults[0].id, 'alpha');
  assert.equal(vaults[0].name, 'Alpha');
  assert.equal(vaults[0].source, '/tmp/obsidian.json');
});

test('builds a manifest with tags links backlinks and tasks', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'aleph-vault-'));
  await fs.mkdir(path.join(root, '.obsidian'));
  await fs.mkdir(path.join(root, 'Projects'));
  await fs.writeFile(path.join(root, 'Projects', 'Aleph.md'), [
    '---',
    'tags: [aleph/project]',
    '---',
    '# Aleph',
    'Links to [[Roadmap]].',
    '- [ ] Ship Obsidian bridge',
  ].join('\n'));
  await fs.writeFile(path.join(root, 'Roadmap.md'), '# Roadmap\n#planning\n');

  const vault = await resolveManualVault(root);
  const manifest = await buildVaultManifest(vault);

  assert.equal(manifest.summary.notes, 2);
  assert.equal(manifest.summary.tags, 2);
  assert.equal(manifest.summary.openTasks, 1);

  const roadmap = manifest.notes.find((note) => note.path === 'Roadmap.md');
  assert.deepEqual(roadmap.backlinks, ['Projects/Aleph.md']);

  const results = searchManifest(manifest, 'planning');
  assert.equal(results[0].note.path, 'Roadmap.md');
});

test('stores paired vault config in overridable location', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'aleph-vault-'));
  const alephHome = await fs.mkdtemp(path.join(os.tmpdir(), 'aleph-home-'));
  await fs.mkdir(path.join(root, '.obsidian'));

  const { vault, configPath } = await savePairedVault({ path: root, name: 'Test Vault' }, { ALEPH_HOME: alephHome }, 'linux');
  const rawConfig = JSON.parse(await fs.readFile(configPath, 'utf8'));

  assert.equal(vault.name, 'Test Vault');
  assert.equal(rawConfig.activeVaultId, vault.id);
  assert.equal(rawConfig.vaults[vault.id].path, root);
});

test('builds Obsidian URI for vault and note', () => {
  const uri = buildObsidianOpenUri({ name: 'My Vault', obsidianId: null }, 'Projects/Aleph.md');

  assert.equal(uri, 'obsidian://open?vault=My+Vault&file=Projects%2FAleph.md');
});
