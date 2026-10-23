//! vl-vcd reader fixture tests. The `rich` fixture is synthetic (exercises
//! the tolerance table in one file); the counter fixture is the real disk
//! artifact `corpus/e2e/51_vcd_counter/expect.vcd` (repo discipline: parser
//! fixtures include on-disk real samples, not just imagined formats).

use vl_vcd::{normalize_width, parse, VcdValue, VcdWriter};

const RICH: &str = "\
$date
  Thu Oct 23 00:00:00 2026
$end
$version
  some other tool
$end
$timescale 1ns $end
$timezero -250 $end
$scope module tb $end
$var reg 8 ! q [7:0] $end
$var wire 1 \" clk $end
$var real 64 # temp $end
$var string 64 $ msg $end
$var wire 4 % alias_a $end
$var wire 4 % alias_b $end
$upscope $end
$enddefinitions $end
$dumpvars
0!
1\"
r3.25 #
shello vcd $
b1010 %
$end
#100
1!
b0101 !
bXX01 %
#200
z!
#250
r-1.5e3 #
sbye now $
h&
#300
w&
";

#[test]
fn rich_fixture_typed_shape() {
    let f = parse(RICH).expect("rich fixture parses");
    // Header.
    assert_eq!(f.timescale.as_deref(), Some("1ns"));
    assert_eq!(f.timezero, Some(-250));
    // Vars: names are scope-joined; index suffix parsed; aliases share id.
    assert_eq!(f.vars.len(), 6);
    assert_eq!(f.vars[0].name, "tb.q");
    assert_eq!(f.vars[0].width, 8);
    assert_eq!(f.vars[0].var_type, "reg");
    assert_eq!(f.vars[0].index, Some((7, 0)));
    assert_eq!(f.vars[2].var_type, "real");
    assert_eq!(f.vars[4].name, "tb.alias_a");
    assert_eq!(f.vars[5].name, "tb.alias_b");
    assert_eq!(f.vars[4].id, f.vars[5].id);
    // Initial values from $dumpvars.
    assert_eq!(f.changes[0].time, 0);
    assert_eq!(f.changes[0].value, VcdValue::Scalar('0'));
    let reals: Vec<_> = f
        .changes
        .iter()
        .filter(|c| matches!(c.value, VcdValue::Real(_)))
        .collect();
    assert_eq!(reals.len(), 2);
    assert!(matches!(&reals[0].value, VcdValue::Real(v) if v == "3.25"));
    assert!(matches!(&reals[1].value, VcdValue::Real(v) if v == "-1.5e3"));
    let strs: Vec<_> = f
        .changes
        .iter()
        .filter(|c| matches!(c.value, VcdValue::Str(_)))
        .collect();
    assert_eq!(strs.len(), 2);
    assert!(matches!(&strs[1].value, VcdValue::Str(v) if v == "bye now"));
    // Uppercase vector digits kept verbatim.
    assert!(f.changes.iter().any(
        |c| matches!(&c.value, VcdValue::Vector(v) if v == "XX01")
    ));
    // Aliases: the b1010 % change lands on BOTH alias vars.
    let alias_changes = f
        .changes
        .iter()
        .filter(|c| c.time == 0 && matches!(&c.value, VcdValue::Vector(v) if v == "1010"))
        .count();
    assert_eq!(alias_changes, 2);
    // Warnings: EVCD strength scalars h/w (VL-W102 x2) — and their id `&`
    // is undeclared, so each also warns unknown-identifier (VL-W100 x2) and
    // the changes are discarded (GTKWave behavior).
    let codes: Vec<_> = f.warnings.iter().map(|w| w.code).collect();
    assert_eq!(codes.iter().filter(|c| **c == "VL-W102").count(), 2);
    assert_eq!(codes.iter().filter(|c| **c == "VL-W100").count(), 2);
    // t_max over the recorded stream (the `&` changes are discarded).
    assert_eq!(f.t_max(), 250);
}

#[test]
fn unknown_id_and_cross_form_warn() {
    let text = "\
$scope module t $end
$var wire 4 ! a $end
$var real 64 # r $end
$enddefinitions $end
#0
b0101 !
b1010 #
r2.5 #
b1111 ?
";
    let f = parse(text).expect("parses");
    let codes: Vec<&str> = f.warnings.iter().map(|w| w.code).collect();
    // b on real -> W103; r on real -> silent; b on unknown id -> W100.
    assert!(codes.contains(&"VL-W100"));
    assert_eq!(codes.iter().filter(|c| **c == "VL-W103").count(), 1);
    // The b-on-real change still lands as a string value.
    assert!(f
        .changes
        .iter()
        .any(|c| matches!(&c.value, VcdValue::Str(v) if v == "1010")));
}

#[test]
fn malformed_errors() {
    // VL-E101: bad #time.
    let e = parse("$enddefinitions $end\n#1e5\n").unwrap_err();
    assert_eq!(e.code, "VL-E101");
    // VL-E104: dumpports in the change section.
    let e = parse("$scope module t $end\n$var wire 1 ! a $end\n$enddefinitions $end\n$dumpports\n")
        .unwrap_err();
    assert_eq!(e.code, "VL-E104");
    // VL-E100: malformed value change (junk token).
    let e = parse(
        "$scope module t $end\n$var wire 1 ! a $end\n$enddefinitions $end\nQ!\n",
    )
    .unwrap_err();
    assert_eq!(e.code, "VL-E100");
    // VL-E102: malformed $var (missing tokens).
    let e = parse("$scope module t $end\n$var wire ! $end\n").unwrap_err();
    assert_eq!(e.code, "VL-E102");
    // VL-E103: malformed $scope.
    let e = parse("$scope module\n").unwrap_err();
    assert_eq!(e.code, "VL-E103");
}

#[test]
fn truncated_eof_warns_but_parses() {
    let text = "$scope module t $end\n$var wire 4 ! a $end\n";
    let f = parse(text).expect("truncated file still parses");
    assert_eq!(f.vars.len(), 1);
    assert!(f.changes.is_empty());
    let codes: Vec<&str> = f.warnings.iter().map(|w| w.code).collect();
    assert!(codes.contains(&"VL-W101"));
}

#[test]
fn normalize_width_contract() {
    // Left-truncate when longer (norm_val contract).
    assert_eq!(normalize_width("1010101", 4), "0101");
    // Left-extend: 0 fill, x/z fill with themselves.
    assert_eq!(normalize_width("101", 8), "00000101");
    assert_eq!(normalize_width("x1", 4), "xxx1");
    assert_eq!(normalize_width("z1", 4), "zzz1");
    assert_eq!(normalize_width("0101", 4), "0101");
}

#[test]
fn writer_output_round_trips_through_reader() {
    // Writer → reader round-trip on the real counter artifact, plus a
    // synthetic writer run.
    let vars = vec![
        ("tb.q".to_string(), vec!["tb".into(), "q".into()], 8u32),
        ("clk".to_string(), vec!["clk".into()], 1u32),
    ];
    let mut w = VcdWriter::new(vars, "1ps");
    w.start();
    let mut vals = std::collections::BTreeMap::new();
    vals.insert("clk".to_string(), "1".to_string());
    vals.insert("tb.q".to_string(), "00000101".to_string());
    w.dump_t0(&vals);
    w.record_change("tb.q", "00000110");
    w.flush_at(10);
    let text = w.finish();
    let f = parse(&text).expect("our own VCD parses");
    assert_eq!(f.vars.len(), 2);
    assert_eq!(f.vars[0].name, "clk");
    assert_eq!(f.timescale.as_deref(), Some("1ps"));
    let q_changes: Vec<_> = f
        .changes
        .iter()
        .filter(|c| f.vars[c.var].name == "tb.q")
        .collect();
    assert_eq!(q_changes.len(), 2);
    assert!(matches!(&q_changes[0].value, VcdValue::Vector(v) if v == "00000101"));
    assert!(matches!(&q_changes[1].value, VcdValue::Vector(v) if v == "00000110"));
}

#[test]
fn real_disk_artifact_parses() {
    // On-disk real sample (repo discipline): the e2e counter's recorded VCD.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/e2e/51_vcd_counter/expect.vcd");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing disk fixture {}: {e}", path.display()));
    let f = parse(&text).expect("expect.vcd parses");
    assert!(!f.vars.is_empty());
    assert!(f.t_max() > 0);
    assert!(f.warnings.is_empty(), "clean artifact: no warnings");
}
