import { spawnSync } from 'node:child_process';

export function executableFromMessages(output, binary) {
  for (const line of output.trim().split('\n').filter(Boolean)) {
    const message = JSON.parse(line);
    if (message.reason === 'compiler-artifact' && message.target.name === binary
      && message.target.kind.includes('bin') && !message.profile.test && message.executable) {
      return message.executable;
    }
  }
  throw new Error(`Cargo did not report an executable for ${binary}`);
}

export function buildBinary(root, args, binary) {
  const result = spawnSync('cargo', ['build', '--message-format=json-render-diagnostics', ...args], {
    cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'], maxBuffer: 16 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`Cargo build failed for ${binary} (${result.status})`);
  return executableFromMessages(result.stdout, binary);
}

export function workspaceDependencyVersions(metadata, names) {
  const members = new Set(metadata.workspace_members);
  const dependencies = new Set(metadata.resolve.nodes
    .filter(node => members.has(node.id)).flatMap(node => node.dependencies));
  return Object.fromEntries(names.map(name => {
    const versions = [...new Set(metadata.packages
      .filter(pkg => pkg.name === name && dependencies.has(pkg.id))
      .map(pkg => pkg.version))].sort();
    if (versions.length !== 1) {
      throw new Error(`Cannot select locked version for ${name}: ${versions.length ? `workspace dependencies use ${versions.join(', ')}` : 'no workspace dependency'}`);
    }
    return [name, versions[0]];
  }));
}
