import { spawn } from 'node:child_process';

export function buildObsidianOpenUri(vault, filePath) {
  const params = new URLSearchParams();
  params.set('vault', vault.obsidianId || vault.name);

  if (filePath) {
    params.set('file', filePath.replace(/\\/g, '/'));
  }

  return `obsidian://open?${params.toString()}`;
}

export async function openExternalUri(uri, options = {}) {
  const platform = options.platform || process.platform;
  const dryRun = Boolean(options.dryRun);

  if (dryRun) {
    return { command: null, uri };
  }

  const command = platform === 'darwin'
    ? 'open'
    : platform === 'win32'
      ? 'cmd'
      : 'xdg-open';
  const args = platform === 'win32'
    ? ['/c', 'start', '', uri]
    : [uri];

  await new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      detached: true,
      stdio: 'ignore',
      windowsHide: true,
    });

    child.on('error', reject);
    child.unref();
    resolve();
  });

  return { command, uri };
}
