// End-to-end browser route and static asset checks for the compiled binary.
// Run through `bun run smoke` (which first builds the CSS and Rust release).

import { afterAll, beforeAll, expect, test } from 'bun:test';
import { spawn, type ChildProcess } from 'node:child_process';
import { createServer } from 'node:net';
import { resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

const projectRoot = resolve(import.meta.dir, '..');
const binary = resolve(projectRoot, 'target/release/cuadb');
let server: ChildProcess | undefined;
let address = '';

async function unusedPort(): Promise<number> {
  return new Promise((resolvePort, reject) => {
    const socket = createServer();
    socket.once('error', reject);
    socket.listen(0, '127.0.0.1', () => {
      const value = socket.address();
      if (!value || typeof value === 'string') {
        socket.close();
        reject(new Error('Could not allocate a TCP port'));
        return;
      }
      socket.close(() => resolvePort(value.port));
    });
  });
}

function launch(port: number): ChildProcess {
  return spawn(binary, [], {
    cwd: projectRoot,
    env: {
      ...process.env,
      CUADB_HOST: '127.0.0.1',
      CUADB_PORT: String(port),
      // The shell must not require credentials, even if the developer has
      // configured a different database connection in their environment.
      CUADB_DATABASE_URL: undefined,
    },
    stdio: 'ignore',
  });
}

async function waitForHealth(origin: string, child: ChildProcess): Promise<void> {
  for (let attempt = 0; attempt < 80; attempt++) {
    if (child.exitCode !== null) {
      throw new Error(`CuaDB exited early with code ${child.exitCode}`);
    }
    try {
      const response = await fetch(`${origin}/health`, {
        signal: AbortSignal.timeout(400),
      });
      if (response.ok && (await response.text()) === 'ok') return;
    } catch {
      // Compiled binary may still be binding its listener.
    }
    await delay(75);
  }
  throw new Error('CuaDB failed to answer /health during smoke setup');
}

beforeAll(async () => {
  const port = await unusedPort();
  address = `http://127.0.0.1:${port}`;
  server = launch(port);
  await waitForHealth(address, server);
}, 15000);

afterAll(() => {
  server?.kill('SIGTERM');
});

test('serves the page, fragment and pure liveness endpoint', async () => {
  const health = await fetch(`${address}/health`);
  expect(health.status).toBe(200);
  expect(await health.text()).toBe('ok');
  expect(health.headers.get('x-request-id')).toMatch(/^cuadb-[a-f0-9]{16}$/);

  const page = await fetch(address);
  expect(page.status).toBe(200);
  expect(await page.text()).toContain('CuaDB');

  const fragment = await fetch(`${address}/fragments/status`);
  expect(fragment.status).toBe(200);
  expect(await fragment.text()).toContain('App shell ready');

  // Catch stale binaries or accidentally dropped connection-status routes.
  // Smoke launches with no database URL, so this does not touch PostgreSQL.
  const connection = await fetch(`${address}/connection/status`);
  expect(connection.status).toBe(200);
  expect(connection.headers.get('cache-control')).toBe('no-store');
  expect(await connection.text()).toContain('Not configured');

  // Catalog routes exist but must not access an unconfigured database.
  const catalog = await fetch(`${address}/catalog/schemas`);
  expect(catalog.status).toBe(200);
  expect(await catalog.text()).toContain('Not configured.');
});

test('serves locally compiled CSS, HTMX and browser JavaScript', async () => {
  for (const asset of ['app.css', 'app.js', 'htmx.min.js']) {
    const response = await fetch(`${address}/static/${asset}`);
    expect(response.status).toBe(200);
    expect((await response.text()).length).toBeGreaterThan(10);
  }
});

test('stops cleanly on Unix SIGTERM without a live database', async () => {
  if (process.platform === 'win32') return;

  const port = await unusedPort();
  const other = launch(port);
  try {
    await waitForHealth(`http://127.0.0.1:${port}`, other);
    const exited = new Promise<number | null>((resolveExit) => {
      other.once('exit', (code) => resolveExit(code));
    });
    other.kill('SIGTERM');
    const code = await new Promise<number | null>((resolveExit) => {
      const timeout = setTimeout(() => {
        other.kill('SIGKILL');
        resolveExit(-1);
      }, 5000);
      void exited.then((exitCode) => {
        clearTimeout(timeout);
        resolveExit(exitCode);
      });
    });
    expect(code).toBe(0);
  } finally {
    other.kill('SIGKILL');
  }
}, 12000);
