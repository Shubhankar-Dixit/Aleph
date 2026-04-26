import { promises as fs } from 'node:fs';
import path from 'node:path';
import { readVaultNotes } from './obsidian.mjs';

export const MANIFEST_DIRECTORY = '.aleph';
export const MANIFEST_FILENAME = 'obsidian-manifest.json';

export function getManifestPath(vaultPath) {
  return path.join(vaultPath, MANIFEST_DIRECTORY, MANIFEST_FILENAME);
}

export async function buildVaultManifest(vault) {
  const notes = await readVaultNotes(vault.path);
  const noteLookup = new Map(notes.map((note) => [note.title.toLowerCase(), note.relativePath]));
  const backlinks = new Map(notes.map((note) => [note.relativePath, []]));
  const tags = new Map();
  const tasks = [];

  for (const note of notes) {
    for (const tag of note.tags) {
      if (!tags.has(tag)) {
        tags.set(tag, []);
      }
      tags.get(tag).push(note.relativePath);
    }

    note.tasks.forEach((task) => {
      tasks.push({
        ...task,
        note: note.relativePath,
      });
    });

    for (const link of note.links) {
      const targetPath = noteLookup.get(link.toLowerCase());
      if (targetPath && backlinks.has(targetPath)) {
        backlinks.get(targetPath).push(note.relativePath);
      }
    }
  }

  return {
    version: 1,
    generatedAt: new Date().toISOString(),
    vault: {
      id: vault.id,
      name: vault.name,
      path: vault.path,
      obsidianId: vault.obsidianId,
    },
    summary: {
      notes: notes.length,
      markdownNotes: notes.filter((note) => note.extension === '.md').length,
      canvases: notes.filter((note) => note.extension === '.canvas').length,
      tags: tags.size,
      links: notes.reduce((total, note) => total + note.links.length, 0),
      tasks: tasks.length,
      openTasks: tasks.filter((task) => !task.done).length,
      words: notes.reduce((total, note) => total + note.wordCount, 0),
    },
    notes: notes.map((note) => ({
      path: note.relativePath,
      title: note.title,
      extension: note.extension,
      tags: note.tags,
      links: note.links,
      backlinks: [...new Set(backlinks.get(note.relativePath) || [])].sort(),
      tasks: note.tasks,
      wordCount: note.wordCount,
      frontmatter: note.frontmatter,
    })),
    tags: Object.fromEntries([...tags.entries()].map(([tag, paths]) => [tag, [...new Set(paths)].sort()])),
    tasks,
  };
}

export async function writeVaultManifest(vault) {
  const manifest = await buildVaultManifest(vault);
  const manifestPath = getManifestPath(vault.path);
  await fs.mkdir(path.dirname(manifestPath), { recursive: true });
  await fs.writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
  return { manifest, manifestPath };
}

export async function readVaultManifest(vault) {
  const manifestPath = getManifestPath(vault.path);
  const raw = await fs.readFile(manifestPath, 'utf8');
  return JSON.parse(raw);
}

export function searchManifest(manifest, query, options = {}) {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  const limit = options.limit || 10;

  if (terms.length === 0) {
    return [];
  }

  return manifest.notes
    .map((note) => {
      const haystack = [
        note.title,
        note.path,
        ...(note.tags || []),
        ...(note.links || []),
        ...(note.backlinks || []),
      ].join(' ').toLowerCase();
      const score = terms.reduce((total, term) => total + (haystack.includes(term) ? 1 : 0), 0);

      return score > 0 ? { note, score } : null;
    })
    .filter(Boolean)
    .sort((left, right) => right.score - left.score || left.note.path.localeCompare(right.note.path))
    .slice(0, limit);
}
