// 2-stage pipelined sample for codegen (register insertion is automatic).
pub fn acc_step(acc: uN[16], x: uN[8]) -> uN[16] {
    acc + (x as uN[16])
}

#[pipeline(stages=2)]
pub fn acc_pipe(x: uN[8], y: uN[8]) -> uN[16] {
    let s1 = (x as uN[16]) + (y as uN[16]);
    let s2 = s1 * (uN[16]:3);
    s2
}
