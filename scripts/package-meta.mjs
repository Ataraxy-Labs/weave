import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptsDirectory = path.dirname(fileURLToPath(import.meta.url));

export const PACKAGE_ROOT = path.resolve(scriptsDirectory, '..');

export const BINARIES = [
  { name: 'weave', artifact: 'weave-cli' },
  { name: 'weave-driver', artifact: 'weave-driver' },
  { name: 'weave-mcp', artifact: 'weave-mcp' },
];

export function getBinaryName(baseName, platform = process.platform) {
  return platform === 'win32' ? `${baseName}.exe` : baseName;
}

export function getVendorDirectory(packageRoot = PACKAGE_ROOT) {
  return path.join(packageRoot, 'vendor');
}

export function getInstalledBinaryPath(baseName, {
  packageRoot = PACKAGE_ROOT,
  platform = process.platform,
} = {}) {
  return path.join(getVendorDirectory(packageRoot), getBinaryName(baseName, platform));
}

export function resolveTarget({
  platform = process.platform,
  arch = process.arch,
} = {}) {
  const key = `${platform}:${arch}`;

  switch (key) {
    case 'linux:x64':
      return 'x86_64-unknown-linux-gnu';
    case 'linux:arm64':
      return 'aarch64-unknown-linux-gnu';
    case 'darwin:arm64':
      return 'aarch64-apple-darwin';
    case 'darwin:x64':
      return 'x86_64-apple-darwin';
    case 'win32:x64':
      return 'x86_64-pc-windows-msvc';
    default:
      throw new Error(
        `Unsupported platform ${key}. Supported: linux/x64, linux/arm64, darwin/arm64, darwin/x64, win32/x64.`,
      );
  }
}

export function resolveReleaseArtifact(artifactPrefix, options = {}) {
  const target = resolveTarget(options);
  const platform = options.platform ?? process.platform;
  const ext = platform === 'win32' ? '.zip' : '.tar.gz';
  return `${artifactPrefix}-${target}${ext}`;
}

export function getReleaseBaseUrl(version, env = process.env) {
  const override = env.WEAVE_RELEASE_BASE_URL?.trim();
  if (override) {
    return override.replace(/\/+$/, '');
  }

  return `https://github.com/Ataraxy-Labs/weave/releases/download/v${version}`;
}

export function getReleaseDownloadUrl(version, artifactPrefix, options = {}) {
  const baseUrl = getReleaseBaseUrl(version, options.env);
  const artifact = resolveReleaseArtifact(artifactPrefix, options);
  return `${baseUrl}/${artifact}`;
}

export async function readPackageVersion(packageRoot = PACKAGE_ROOT) {
  const packageJsonPath = path.join(packageRoot, 'package.json');
  const packageJson = JSON.parse(await fs.readFile(packageJsonPath, 'utf8'));
  return packageJson.version;
}

export async function readCargoPackageVersion(
  manifestPath = path.join(PACKAGE_ROOT, 'crates', 'weave-core', 'Cargo.toml'),
) {
  const cargoToml = await fs.readFile(manifestPath, 'utf8');
  const versionMatch = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);

  if (!versionMatch) {
    throw new Error(`Could not find version in ${manifestPath}`);
  }

  return versionMatch[1];
}

export async function syncPackageVersion({
  packageRoot = PACKAGE_ROOT,
  version,
} = {}) {
  const resolvedVersion = version ?? (await readCargoPackageVersion());
  const packageJsonPath = path.join(packageRoot, 'package.json');
  const packageJson = JSON.parse(await fs.readFile(packageJsonPath, 'utf8'));
  const changed = packageJson.version !== resolvedVersion;

  if (changed) {
    packageJson.version = resolvedVersion;
    await fs.writeFile(
      packageJsonPath,
      `${JSON.stringify(packageJson, null, 2)}\n`,
      'utf8',
    );
  }

  return {
    changed,
    version: resolvedVersion,
  };
}
