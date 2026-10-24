// Constants for the dev scripts. Change a value here, never at a call site.

/**
 * Port Vite serves on. vite.config.ts (server.port) and src-tauri/tauri.conf.json
 * (devUrl) hard-code the same number, so change all three together.
 */
export const DEV_PORT = 47821;
export const DEV_URL = `http://localhost:${DEV_PORT}`;

/** Time a process gets after SIGTERM before SIGKILL. Shorter than with-env.sh's one second, which would KILL this script. */
export const KILL_GRACE_MS = 500;

/** How long Vite may take to answer on the port before the script gives up. */
export const VITE_READY_TIMEOUT_MS = 60_000;

/** Gap between port probes while Vite starts. */
export const PORT_RETRY_MS = 250;

/**
 * A build slot is freed after this long even when no marker line appeared, so a build
 * that prints nothing recognisable cannot block the queue for good. Long enough for a
 * cold Rust build on the 8 GB Mac.
 */
export const BUILD_SLOT_TIMEOUT_MS = 30 * 60_000;
