// Pipeline sample: stages come from codegen flags (--pipeline_stages),
// not a DSLX attribute (the #[pipeline] attribute no longer exists in
// current DSLX — verified against the tool's own parse error, 2026-10-23).
pub fn acc_pipe(x: uN[8], y: uN[8]) -> uN[16] {
    // `s1`/`s2`-style short names collide with current DSLX keywords
    // (verified via tool parse error, 2026-10-23) — use descriptive names.
    let sum_xy = (x as uN[16]) + (y as uN[16]);
    let scaled = sum_xy * (uN[16]:3);
    scaled
}
