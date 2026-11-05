/**
 * Root Bun command dispatcher.
 *
 * Bun manages frontend tooling only; the app itself is built and served by Rust.
 * On first invocation, missing JS dependencies are installed automatically.
 */
import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readdirSync, watch, type FSWatcher } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const path = (...parts: string[]) => resolve(root, ...parts);
const bin = (name: string) => path('node_modules', '.bin', name);
const tailwindArgs = ['-i', 'assets/css/app.css', '-o', 'public/app.css'];
const prettierFiles = [
  'assets/css/app.css',
  'assets/js/app.js',
  'assets/js/theme.js',
  'assets/pwa/sw.js',
  'assets/pwa/site.webmanifest',
  'scripts/**/*.ts',
  'package.json',
  'tsconfig.json',
  '.prettierrc.json',
];

/** Execute a command synchronously and propagate its exit status. */
function run(command: string, args: string[]): void {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit', env: process.env });
  if (result.error) {
    throw new Error(`Cannot run ${command}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

/** Install dependencies lazily so each root command works after a fresh clone. */
function ensureFrontend(): void {
  if (
    !existsSync(bin('tailwindcss')) ||
    !existsSync(path('node_modules/htmx.org/dist/htmx.min.js'))
  ) {
    console.log('Installing frontend tooling with Bun…');
    run('bun', ['install', '--frozen-lockfile']);
  }
}

function ensureRust(): void {
  const result = spawnSync('cargo', ['--version'], { cwd: root, stdio: 'ignore' });
  if (result.error || result.status !== 0) {
    throw new Error('Rust/Cargo is required. Install rustup, rustfmt and clippy (see README.md).');
  }
}

/** Build CSS, then bundle only browser assets that the Rust server will serve. */
function buildAssets(release = false): void {
  ensureFrontend();
  mkdirSync(path('public'), { recursive: true });
  run(bin('tailwindcss'), release ? [...tailwindArgs, '--minify'] : tailwindArgs);
  copyFileSync(path('node_modules/htmx.org/dist/htmx.min.js'), path('public/htmx.min.js'));
  copyFileSync(path('assets/js/app.js'), path('public/app.js'));
  copyFileSync(path('assets/js/theme.js'), path('public/theme.js'));
  copyFileSync(path('assets/pwa/sw.js'), path('public/sw.js'));
  copyFileSync(path('assets/pwa/site.webmanifest'), path('public/site.webmanifest'));
  copyFileSync(path('assets/icons/favicon.svg'), path('public/favicon.svg'));
  copyFileSync(path('assets/icons/icon-192.png'), path('public/icon-192.png'));
  copyFileSync(path('assets/icons/icon-512.png'), path('public/icon-512.png'));
}

/**
 * Explicit, one-time Cargo dependency sync after changing Cargo.toml.
 *
 * Ordinary commands remain locked and reproducible. Commit the generated
 * Cargo.lock together with Cargo.toml before creating a release image.
 */
function syncDependencies(): void {
  ensureRust();
  run('cargo', ['check', '--all-targets']);
  console.log('Cargo dependency resolution complete. Review and commit Cargo.lock.');
}

function build(): void {
  ensureRust();
  buildAssets(true);
  run('cargo', ['build', '--locked', '--release']);
}

function format(check: boolean): void {
  ensureRust();
  ensureFrontend();
  run('cargo', check ? ['fmt', '--all', '--', '--check'] : ['fmt', '--all']);
  run(bin('prettier'), [check ? '--check' : '--write', ...prettierFiles]);
}

function check(): void {
  ensureRust();
  ensureFrontend();
  run('cargo', ['fmt', '--all', '--', '--check']);
  run(bin('prettier'), ['--check', ...prettierFiles]);
  run('cargo', ['check', '--locked', '--all-targets', '--all-features']);
  run('cargo', ['clippy', '--locked', '--all-targets', '--all-features', '--', '-D', 'warnings']);
  run(bin('tsc'), ['--noEmit']);
}

function test(): void {
  ensureRust();
  // The shell tests also exercise the CSS and JS served by Axum.
  buildAssets();
  run('cargo', ['test', '--locked', '--all-targets']);
  run('bun', ['test', 'scripts/roadmap/spec.test.ts']);
  run(bin('tsc'), ['--noEmit']);
}

/** Build and probe a running release process without requiring a database. */
function smoke(): void {
  build();
  run('bun', ['test', 'scripts/smoke.test.ts']);
}

/** Enumerate directories for portable, non-recursive file watchers. */
function listDirectories(directory: string): string[] {
  if (!existsSync(directory)) return [];
  return [
    directory,
    ...readdirSync(directory, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .flatMap((entry) => listDirectories(resolve(directory, entry.name))),
  ];
}

function signalGroup(child: ChildProcess, signal: NodeJS.Signals): void {
  if (!child.pid) return;
  try {
    if (process.platform === 'win32') child.kill(signal);
    else process.kill(-child.pid, signal);
  } catch {
    // An already exited child does not require cleanup.
  }
}

function terminate(child: ChildProcess): void {
  signalGroup(child, 'SIGTERM');
}

/** SIGTERM the child's process group, wait for it to exit, SIGKILL after `graceMs`. */
function stopAndWait(child: ChildProcess, graceMs = 3000): Promise<void> {
  return new Promise((resolveStop) => {
    if (child.exitCode !== null || child.signalCode !== null) return resolveStop();
    const force = setTimeout(() => signalGroup(child, 'SIGKILL'), graceMs);
    child.once('exit', () => {
      clearTimeout(force);
      resolveStop();
    });
    terminate(child);
  });
}

/** Hot-reload Rust and Askama files; Tailwind watches styles independently. */
function dev(): void {
  ensureRust();
  buildAssets();

  console.log(
    `Starting CuaDB at http://${process.env.CUADB_HOST ?? '127.0.0.1'}:${process.env.CUADB_PORT ?? '4848'}`,
  );
  const css = spawn(bin('tailwindcss'), [...tailwindArgs, '--watch'], {
    cwd: root,
    stdio: 'inherit',
    detached: process.platform !== 'win32',
  });
  const launchRust = () =>
    spawn('cargo', ['run', '--locked'], {
      cwd: root,
      stdio: 'inherit',
      detached: process.platform !== 'win32',
      // A short drain keeps restarts and Ctrl+C from leaving the old server on the port.
      env: { CUADB_SHUTDOWN_GRACE_MS: '500', ...process.env },
    });
  let rust = launchRust();
  let pending: ReturnType<typeof setTimeout> | undefined;
  let restarting = false;

  const restart = () => {
    if (pending) clearTimeout(pending);
    pending = setTimeout(async () => {
      if (restarting) return restart();
      restarting = true;
      console.log('Restarting Rust server after source change…');
      // The old server must release the port before the new one binds it.
      await stopAndWait(rust);
      rust = launchRust();
      restarting = false;
    }, 350);
  };
  const watchers: FSWatcher[] = listDirectories(path('src'))
    .concat(listDirectories(path('templates')))
    .map((directory) => watch(directory, restart));

  watchers.push(
    watch(path('assets/js'), () => {
      copyFileSync(path('assets/js/app.js'), path('public/app.js'));
      copyFileSync(path('assets/js/theme.js'), path('public/theme.js'));
    }),
  );

  let closing = false;
  const shutdown = async () => {
    if (closing) return;
    closing = true;
    if (pending) clearTimeout(pending);
    watchers.forEach((watcher) => watcher.close());
    await Promise.all([stopAndWait(css, 1500), stopAndWait(rust)]);
    process.exit(0);
  };
  // Ctrl+C, kill, and a closed terminal must all release the port.
  for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP'] as const) process.on(signal, shutdown);
  // Last resort if this script is leaving for another reason.
  process.on('exit', () => {
    signalGroup(rust, 'SIGKILL');
    signalGroup(css, 'SIGKILL');
  });
}

function start(): void {
  // A previous release build may be stale after a source change.
  build();
  run(path('target/release/cuadb'), []);
}

function main(): void {
  const action = process.argv[2];
  switch (action) {
    case 'deps:sync':
      syncDependencies();
      break;
    case 'setup':
      ensureRust();
      buildAssets();
      run('cargo', ['fetch', '--locked']);
      console.log('Setup complete. Try: bun run dev');
      break;
    case 'dev':
      dev();
      break;
    case 'check':
      check();
      break;
    case 'build':
      build();
      break;
    case 'start':
      start();
      break;
    case 'format':
      format(false);
      break;
    case 'format:check':
      format(true);
      break;
    case 'test':
      test();
      break;
    case 'smoke':
      smoke();
      break;
    case 'assets':
      buildAssets(true);
      break;
    default:
      console.error(
        'Usage: bun run <setup|deps:sync|dev|check|build|start|format|format:check|test|smoke>',
      );
      process.exit(1);
  }
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
