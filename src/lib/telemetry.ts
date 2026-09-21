import { commands } from "../bindings";

/**
 * Fire-and-forget usage counters for UI-only interactions (navigation clicks,
 * view toggles, export choices). Rust-side actions count themselves.
 *
 * Names are dotted lowercase identifiers (`nav.prev`, `export.format.json`);
 * anything else is dropped by the backend. Never pass user data as a name.
 * Counters are summed locally and sent once a day — see `src-tauri/src/telemetry.rs`
 * and the Telemetry section of the README for exactly what leaves the machine.
 */
export function track(name: string): void {
  commands.telemetryTrack(name).catch(() => {
    // Telemetry must never surface to the user.
  });
}

/** Record current state (not a count), e.g. `setGauge("lang", "da")`. */
export function setGauge(name: string, value: string): void {
  commands.telemetrySetGauge(name, value).catch(() => {
    // Telemetry must never surface to the user.
  });
}
