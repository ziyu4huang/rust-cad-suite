// Small combinational sample: 3-input adder tree, width-promoting.
fn add3(x: uN[8], y: uN[8], z: uN[8]) -> uN[9] {
    (x as uN[9]) + (y as uN[9]) + (z as uN[9])
}

#[test]
fn add3_test() {
    assert_eq(add3(u8:1, u8:2, u8:3), u9:6);
}
