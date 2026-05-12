import fs from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import {
  BINARIES,
  getBinaryName,
  getInstalledBinaryPath,
  getReleaseBaseUrl,
  getReleaseDownloadUrl,
  readPackageVersion,
  resolveReleaseArtifact,
} from './package-meta.mjs';
import { verifyChecksum } from './verify-checksum.mjs';

async function downloadFile(url, destinationPath) {
  const response = await fetch(url, {
    headers: {
      'user-agent': '@ataraxy-labs/weave npm installer',
    },
    redirect: 'follow',
  });

  if (!response.ok) {
    throw new Error(`Download failed with ${response.status} ${response.statusText}`);
  }

  if (!response.body) {
    throw new Error('Download response did not include a body');
  }

  await pipeline(
    Readable.fromWeb(response.body),
    createWriteStream(destinationPath),
  );
}

function extractArchive(archivePath, outputDirectory) {
  const result = spawnSync('tar', ['-xzf', archivePath, '-C', outputDirectory], {
    stdio: 'pipe',
  });

  if (result.error) {
    throw new Error(`Failed to extract archive: ${result.error.message}`);
  }

  if (result.status !== 0) {
    const stderr = result.stderr?.toString('utf8').trim();
    throw new Error(
      `Failed to extract archive with tar${stderr ? `: ${stderr}` : ''}`,
    );
  }
}

function extractZip(zipPath, outputDirectory) {
  const result = spawnSync('powershell', [
    '-Command',
    `Expand-Archive -Path '${zipPath}' -DestinationPath '${outputDirectory}' -Force`,
  ], { stdio: 'pipe' });

  if (result.error || result.status !== 0) {
    throw new Error('Failed to extract zip archive');
  }
}

async function installBinary(sourcePath, destinationPath) {
  await fs.mkdir(path.dirname(destinationPath), { recursive: true });
  await fs.copyFile(sourcePath, destinationPath);

  if (process.platform !== 'win32') {
    await fs.chmod(destinationPath, 0o755);
  }
}

async function installFromRelease(binary, version, releaseBaseUrl, temporaryDirectory) {
  const { name, artifact } = binary;
  const archiveName = resolveReleaseArtifact(artifact);
  const releaseUrl = getReleaseDownloadUrl(version, artifact);
  const archivePath = path.join(temporaryDirectory, archiveName);
  const destinationPath = getInstalledBinaryPath(name);

  console.log(`  Downloading ${name}...`);
  await downloadFile(releaseUrl, archivePath);
  await verifyChecksum(archivePath, archiveName, releaseBaseUrl);

  if (archiveName.endsWith('.zip')) {
    extractZip(archivePath, temporaryDirectory);
  } else {
    extractArchive(archivePath, temporaryDirectory);
  }

  const binaryFileName = getBinaryName(name);
  await installBinary(
    path.join(temporaryDirectory, binaryFileName),
    destinationPath,
  );
}

async function main() {
  if (process.env.WEAVE_SKIP_DOWNLOAD === '1') {
    console.log('Skipping weave binary download because WEAVE_SKIP_DOWNLOAD=1.');
    return;
  }

  const version = await readPackageVersion();
  const releaseBaseUrl = getReleaseBaseUrl(version);
  const temporaryDirectory = await fs.mkdtemp(
    path.join(os.tmpdir(), 'weave-install-'),
  );

  try {
    console.log(`Installing weave v${version} binaries...`);

    for (const binary of BINARIES) {
      await installFromRelease(binary, version, releaseBaseUrl, temporaryDirectory);
    }

    console.log('Installed weave, weave-driver, and weave-mcp.');
  } finally {
    await fs.rm(temporaryDirectory, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(`Failed to install weave: ${error.message}`);
  process.exit(1);
});
