import { isAbsolute, relative, resolve, sep } from 'node:path';

/** Converts local coordinates; host file access remains a separate check. */
export function portableFile(file: string, localRoot: string | undefined): string {
  if (!localRoot) throw new Error('Configure localRoot before sending local file coordinates');
  if ((sep === '/' && file.includes('\\')) || (/^[a-z]:/i.test(file) && !isAbsolute(file))) {
    throw new Error('Local file is outside the portable project path space');
  }
  const root = resolve(localRoot);
  const target = resolve(root, file);
  const path = relative(root, target);
  if (!path || path === '..' || path.startsWith('..' + sep) || isAbsolute(path)) {
    throw new Error('Local file is outside the configured project root');
  }
  return path.split(sep).join('/');
}
