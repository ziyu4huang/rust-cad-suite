// Minimal VCD parser + waveform data prep for the demo page.
// Understands the subset rverilator emits: $scope/$var/$enddefinitions,
// #t steps, b<vector> <id> and scalar 0/1/x/z<id> changes.

export function parseVcd(text) {
  const idName = new Map(); // id -> { name, width, scope }
  const signals = [];       // { id, name, width }
  let time = 0;
  const changes = new Map(); // id -> [[t, valueString], ...]
  let inDefs = true;
  const scope = [];
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line) continue;
    if (inDefs) {
      if (line.startsWith("$scope")) {
        scope.push(line.split(/\s+/)[2]);
      } else if (line.startsWith("$upscope")) {
        scope.pop();
      } else if (line.startsWith("$var")) {
        const [, , width, id, ...rest] = line.split(/\s+/);
        const base = rest[rest.length - 2] === "" ? rest[0] : rest[0];
        const name = [...scope, rest[0]].join(".");
        idName.set(id, { name, width: Number(width) });
        signals.push({ id, name, width: Number(width) });
      } else if (line.startsWith("$enddefinitions")) {
        inDefs = false;
      }
    } else if (line.startsWith("#")) {
      time = Number(line.slice(1));
    } else if (line.startsWith("b")) {
      const sp = line.indexOf(" ");
      const value = line.slice(1, sp);
      const id = line.slice(sp + 1);
      push(changes, id, time, value);
    } else if (/^[01xz]$/.test(line[0]) && line.length >= 2) {
      push(changes, line.slice(1), time, line[0]);
    }
  }
  function push(m, id, t, v) {
    if (!m.has(id)) m.set(id, []);
    const arr = m.get(id);
    // collapse repeats (last write in a slot wins — mirrors the writer)
    if (arr.length && arr[arr.length - 1][1] === v && arr[arr.length - 1][0] === t) {
      arr[arr.length - 1] = [t, v];
    } else if (!arr.length || arr[arr.length - 1][0] !== t) {
      arr.push([t, v]);
    } else {
      arr[arr.length - 1] = [t, v];
    }
  }
  signals.sort((a, b) => a.name.localeCompare(b.name));
  for (const s of signals) s.changes = changes.get(s.id) ?? [];
  const tMax = signals.reduce((m, s) => Math.max(m, s.changes.length ? s.changes[s.changes.length - 1][0] : 0), 0);
  return { signals, tMax };
}

/// Compute per-signal value just before each render grid line.
export function valueAt(sig, t) {
  let v = "x";
  for (const [ct, cv] of sig.changes) {
    if (ct > t) break;
    v = cv;
  }
  return v;
}
