//! HELP_SHORT:yosys -H 录制的 306 条命令短描述(gen_help_desc.sh 机械生成;禁手改)。
//!
//! 来源=oracle `yosys -H` 实跑(scripts/gen_help_desc.sh,默认
//! vendor-reference/suite-dl/oss-cad-suite/bin/yosys;录制日期见 git log
//! 本檔);录制序=-H 输出序=std::map 字典序(register.cc:42),
//! **禁运行时排序**(RYOSYS-MVP §10.3-4)。消费面=bare help 名单行
//! (`    %-20s %s` 形;名对齐 20 列、名长>20 不截断)。
pub const HELP_SHORT: &[(&str, &str)] = &[
    ("abc", "use ABC for technology mapping"),
    ("abc9", "use ABC9 for technology mapping"),
    ("abc9_exe", "use ABC9 for technology mapping"),
    ("abc9_ops", "helper functions for ABC9"),
    ("abc_new", "(experimental) use ABC for SC technology mapping (new)"),
    ("abc_ops_reintegrate", "reintegrate ABC mapped design into module"),
    ("abstract", "replace signals with abstract values during formal verification"),
    ("add", "add objects to the design"),
    ("aigmap", "map logic to and-inverter-graph circuit"),
    ("alumacc", "extract ALU and MACC cells"),
    ("anlogic_eqn", "Anlogic: Calculate equations for luts"),
    ("anlogic_fixcarry", "Anlogic: fix carry chain"),
    ("arith_tree", "convert add/sub/macc/alu chains to carry-save adder trees"),
    ("assertpmux", "adds asserts for parallel muxes"),
    ("async2sync", "convert async FF inputs to sync circuits"),
    ("attrmap", "renaming attributes"),
    ("attrmvcp", "move or copy attributes from wires to driving cells"),
    ("autoname", "automatically assign names to objects"),
    ("blackbox", "convert modules into blackbox modules"),
    ("bmuxmap", "transform $bmux cells to trees of $mux cells"),
    ("booth", "map $mul cells to Booth multipliers"),
    ("box_derive", "derive box modules"),
    ("bufnorm", "(experimental) convert design into buffered-normalized form"),
    ("bugpoint", "minimize testcases"),
    ("bwmuxmap", "replace $bwmux cells with equivalent logic"),
    ("cd", "a shortcut for 'select -module <name>'"),
    ("cellmatch", "match cells to their targets in cell library"),
    ("check", "check for obvious problems in the design"),
    ("check_mem", "check for obvious memory problems in the design"),
    ("chformal", "change formal constraints of the design"),
    ("chparam", "re-evaluate modules with new parameters"),
    ("chtype", "change type of cells in the design"),
    ("clean", "remove unused cells and wires"),
    ("clean_zerowidth", "clean zero-width connections from the design"),
    ("clk2fflogic", "convert clocked FFs to generic $ff cells"),
    ("clkbufmap", "insert clock buffers on clock networks"),
    ("clockgate", "extract clock gating out of flip flops"),
    ("connect", "create or remove connections"),
    ("connect_rpc", "connect to RPC frontend"),
    ("connwrappers", "match width of input-output port pairs"),
    ("constmap", "technology mapping of coarse constant value"),
    ("copy", "copy modules in the design"),
    ("cutpoint", "adds formal cut points to the design"),
    ("debug", "run command with debug log messages enabled"),
    ("delete", "delete objects in the design"),
    ("deminout", "demote inout ports to input or output"),
    ("demuxmap", "transform $demux cells to $eq + $mux cells"),
    ("design", "save, restore and reset current design"),
    ("design_equal", "check if two designs are the same"),
    ("dffinit", "set INIT param on FF cells"),
    ("dfflegalize", "convert FFs and LATCHes to types supported by the target"),
    ("dfflibmap", "technology mapping of flip-flops and latches"),
    ("dffunmap", "unmap clock enable and synchronous reset from FFs"),
    ("dft_tag", "create tagging logic for data flow tracking"),
    ("dump", "print parts of the design in RTLIL format"),
    ("echo", "turning echoing back of commands on and off"),
    ("edgetypes", "list all types of edges in selection"),
    ("efinix_fixcarry", "Efinix: fix carry chain"),
    ("equiv_add", "add a $equiv cell"),
    ("equiv_induct", "proving $equiv cells using temporal induction"),
    ("equiv_make", "prepare a circuit for equivalence checking"),
    ("equiv_mark", "mark equivalence checking regions"),
    ("equiv_miter", "extract miter from equiv circuit"),
    ("equiv_opt", "prove equivalence for optimized circuit"),
    ("equiv_purge", "purge equivalence checking module"),
    ("equiv_remove", "remove $equiv cells"),
    ("equiv_simple", "try proving simple $equiv instances"),
    ("equiv_status", "print status of equivalent checking module"),
    ("equiv_struct", "structural equivalence checking"),
    ("eval", "evaluate the circuit given an input"),
    ("example_dt", "drivertools example"),
    ("exec", "execute commands in the operating system shell"),
    ("expose", "convert internal signals to module ports"),
    ("extract", "find subcircuits and replace them with cells"),
    ("extract_counter", "Extract GreenPak4 counter cells"),
    ("extract_fa", "find and extract full/half adders"),
    ("extract_reduce", "converts gate chains into $reduce_* cells"),
    ("extractinv", "extract explicit inverter cells for invertible cell pins"),
    ("flatten", "flatten design"),
    ("fmcombine", "combine two instances of a cell into one"),
    ("fminit", "set init values/sequences for formal"),
    ("formalff", "prepare FFs for formal"),
    ("freduce", "perform functional reduction"),
    ("fsm", "extract and optimize finite state machines"),
    ("fsm_detect", "finding FSMs in design"),
    ("fsm_expand", "expand FSM cells by merging logic into it"),
    ("fsm_export", "exporting FSMs to KISS2 files"),
    ("fsm_extract", "extracting FSMs in design"),
    ("fsm_info", "print information on finite state machines"),
    ("fsm_map", "mapping FSMs to basic logic"),
    ("fsm_opt", "optimize finite state machines"),
    ("fsm_recode", "recoding finite state machines"),
    ("fst2tb", "generate testbench out of fst file"),
    ("future", "resolve future sampled value functions"),
    ("gatemate_foldinv", "fold inverters into Gatemate LUT trees"),
    ("glift", "create GLIFT models and optimization problems"),
    ("help", "display help messages"),
    ("hierarchy", "check, expand and clean up design hierarchy"),
    ("hilomap", "technology mapping of constant hi- and/or lo-drivers"),
    ("history", "show last interactive commands"),
    ("ice40_braminit", "iCE40: perform SB_RAM40_4K initialization from file"),
    ("ice40_dsp", "iCE40: map multipliers"),
    ("ice40_opt", "iCE40: perform simple optimizations"),
    ("ice40_wrapcarry", "iCE40: wrap carries"),
    ("icell_liberty", "write Liberty interfaces for used internal cells"),
    ("insbuf", "insert buffer cells for connected wires"),
    ("internal_stats", "print internal statistics"),
    ("iopadmap", "technology mapping of i/o pads (or buffers)"),
    ("jny", "write design and metadata"),
    ("json", "write design in JSON format"),
    ("keep_hierarchy", "selectively add the keep_hierarchy attribute"),
    ("lattice_dsp_nexus", "Lattice Nexus DSP inference"),
    ("lattice_gsr", "Lattice: handle GSR"),
    ("libcache", "control caching of technology library data parsed from liberty files"),
    ("license", "print license terms"),
    ("linecoverage", "report coverage information"),
    ("log", "print text and log files"),
    ("logger", "set logger properties"),
    ("ls", "list modules or objects in modules"),
    ("ltp", "print longest topological path"),
    ("lut2bmux", "convert $lut to $bmux"),
    ("lut2mux", "convert $lut to $mux/$_MUX_"),
    ("maccmap", "mapping macc cells"),
    ("mcm", "replace constant multipliers with shared adder graphs"),
    ("memory", "translate memories to basic cells"),
    ("memory_bmux2rom", "convert muxes to ROMs"),
    ("memory_bram", "map memories to block rams"),
    ("memory_collect", "creating multi-port memory cells"),
    ("memory_dff", "merge input/output DFFs into memory read ports"),
    ("memory_libmap", "map memories to cells"),
    ("memory_map", "translate multiport memories to basic cells"),
    ("memory_memx", "emulate vlog sim behavior for mem ports"),
    ("memory_narrow", "split up wide memory ports"),
    ("memory_nordff", "extract read port FFs from memories"),
    ("memory_share", "consolidate memory ports"),
    ("memory_unpack", "unpack multi-port memory cells"),
    ("microchip_dffopt", "MICROCHIP: optimize FF control signal usage"),
    ("microchip_dsp", "MICROCHIP: pack resources into DSPs"),
    ("miter", "automatically create a miter circuit"),
    ("mutate", "generate or apply design mutations"),
    ("muxcover", "cover trees of MUX cells with wider MUXes"),
    ("muxpack", "$mux/$pmux cascades to $pmux"),
    ("nx_carry", "NanoXplore: create carry cells"),
    ("onehot", "optimize $eq cells for onehot signals"),
    ("opensta", "run OpenSTA"),
    ("opt", "perform simple optimizations"),
    ("opt_balance_tree", "$and/$or/$xor/$add/$mul cascades to trees"),
    ("opt_clean", "remove unused cells and wires"),
    ("opt_demorgan", "Optimize reductions with DeMorgan equivalents"),
    ("opt_dff", "perform DFF optimizations"),
    ("opt_expr", "perform const folding and simple expression rewriting"),
    ("opt_ffinv", "push inverters through FFs"),
    ("opt_hier", "perform cross-boundary optimization"),
    ("opt_lut", "optimize LUT cells"),
    ("opt_lut_ins", "discard unused LUT inputs"),
    ("opt_mem", "optimize memories"),
    ("opt_mem_feedback", "convert memory read-to-write port feedback paths to write enables"),
    ("opt_mem_priority", "remove priority relations between write ports that can never collide"),
    ("opt_mem_widen", "optimize memories where all ports are wide"),
    ("opt_merge", "consolidate identical cells"),
    ("opt_muxtree", "eliminate dead trees in multiplexer trees"),
    ("opt_reduce", "simplify large MUXes and AND/OR gates"),
    ("opt_share", "merge mutually exclusive cells of the same type that share an input signal"),
    ("optbarriers", "(experimental) insert optimization barriers"),
    ("paramap", "renaming cell parameters"),
    ("peepopt", "collection of peephole optimizers"),
    ("plugin", "load and list loaded plugins"),
    ("pmux2shiftx", "transform $pmux cells to $shiftx cells"),
    ("pmuxtree", "transform $pmux cells to trees of $mux cells"),
    ("portarcs", "derive port arcs for propagation delay"),
    ("portlist", "list (top-level) ports"),
    ("prep", "generic synthesis script"),
    ("printattrs", "print attributes of selected objects"),
    ("proc", "translate processes to netlists"),
    ("proc_arst", "detect asynchronous resets"),
    ("proc_clean", "remove empty parts of processes"),
    ("proc_dff", "extract flip-flops from processes"),
    ("proc_dlatch", "extract latches from processes"),
    ("proc_init", "convert initial block to init attributes"),
    ("proc_memwr", "extract memory writes from processes"),
    ("proc_mux", "convert decision trees to multiplexers"),
    ("proc_prune", "remove redundant assignments"),
    ("proc_rmdead", "eliminate dead trees in decision trees"),
    ("proc_rom", "convert switches to ROMs"),
    ("qbfsat", "solve a 2QBF-SAT problem in the circuit"),
    ("ql_bram_merge", "Infers QuickLogic k6n10f BRAM pairs that can operate independently"),
    ("ql_bram_types", "Change TDP36K type to subtypes"),
    ("ql_dsp_io_regs", "change types of QL_DSP2 depending on configuration"),
    ("ql_dsp_macc", "infer QuickLogic multiplier-accumulator DSP cells"),
    ("ql_dsp_simd", "merge QuickLogic K6N10f DSP pairs to operate in SIMD mode"),
    ("ql_ioff", "Infer I/O FFs for qlf_k6n10f architecture"),
    ("raise_error", "raise errors from attributes"),
    ("read", "load HDL designs"),
    ("read_aiger", "read AIGER file"),
    ("read_blif", "read BLIF file"),
    ("read_json", "read JSON file"),
    ("read_liberty", "read cells from liberty file"),
    ("read_rtlil", "read modules from RTLIL file"),
    ("read_slang", "read SystemVerilog (slang)"),
    ("read_verilog", "read modules from Verilog file"),
    ("read_verilog_file_list", "parse a Verilog file list"),
    ("recover_names", "Execute a lossy mapping command and recover original netnames"),
    ("rename", "rename object in the design"),
    ("rmports", "remove module ports with no connections"),
    ("sat", "solve a SAT problem in the circuit"),
    ("scatter", "add additional intermediate nets"),
    ("scc", "detect strongly connected components (logic loops)"),
    ("scratchpad", "get/set values in the scratchpad"),
    ("script", "execute commands from file or wire"),
    ("sdc_expand", "expand SDC design with opensta"),
    ("select", "modify and view the list of selected objects"),
    ("setattr", "set/unset attributes on objects"),
    ("setenv", "set an environment variable"),
    ("setparam", "set/unset parameters on objects"),
    ("setundef", "replace undef values with defined constants"),
    ("share", "perform sat-based resource sharing"),
    ("shell", "enter interactive command mode"),
    ("show", "generate schematics using graphviz"),
    ("shregmap", "map shift registers"),
    ("shuffle", "deterministically reorder design objects"),
    ("sim", "simulate the circuit"),
    ("simplemap", "mapping simple coarse-grain cells"),
    ("slang_defaults", "set default options for read_slang"),
    ("slang_version", "display revision of slang frontend"),
    ("sort", "sort the design objects"),
    ("splice", "create explicit splicing cells"),
    ("splitcells", "split up multi-bit cells"),
    ("splitnets", "split up multi-bit nets"),
    ("sta", "perform static timing analysis"),
    ("stat", "print some statistics"),
    ("submod", "moving part of a module to a new submodule"),
    ("supercover", "add hi/lo cover cells for each wire bit"),
    ("symfpu", "SymFPU based floating point netlist generator"),
    ("symfpu_convert", "SymFPU based floating point conversion netlist generator"),
    ("synth", "generic synthesis script"),
    ("synth_achronix", "synthesis for Achronix Speedster22i FPGAs."),
    ("synth_analogdevices", "synthesis for Analog Devices FPGAs"),
    ("synth_anlogic", "synthesis for Anlogic FPGAs"),
    ("synth_easic", "synthesis for eASIC platform"),
    ("synth_ecp5", "synthesis for ECP5 FPGAs"),
    ("synth_efinix", "synthesis for Efinix FPGAs"),
    ("synth_fabulous", "FABulous synthesis script"),
    ("synth_gatemate", "synthesis for Cologne Chip GateMate FPGAs"),
    ("synth_gowin", "synthesis for Gowin FPGAs"),
    ("synth_ice40", "synthesis for iCE40 FPGAs"),
    ("synth_intel", "synthesis for Intel (Altera) FPGAs."),
    ("synth_intel_alm", "synthesis for ALM-based Intel (Altera) FPGAs."),
    ("synth_lattice", "synthesis for Lattice FPGAs"),
    ("synth_microchip", "synthesis for Microchip FPGAs"),
    ("synth_nanoxplore", "synthesis for NanoXplore FPGAs"),
    ("synth_nexus", "synthesis for Nexus FPGAs"),
    ("synth_quicklogic", "Synthesis for QuickLogic FPGAs"),
    ("synth_sf2", "synthesis for SmartFusion2 and IGLOO2 FPGAs"),
    ("synth_xilinx", "synthesis for Xilinx FPGAs"),
    ("synthprop", "synthesize SVA properties"),
    ("techmap", "generic technology mapper"),
    ("tee", "redirect command output to file"),
    ("test_abcloop", "automatically test handling of loops in abc command"),
    ("test_autotb", "generate simple test benches"),
    ("test_cell", "automatically test the implementation of a cell type"),
    ("test_generic", "test the generic compute graph"),
    ("test_kogge_stone", "build adders for Kogge-Stone equivalence testing"),
    ("test_pmgen", "test pass for pmgen"),
    ("test_select", "call internal selection methods on design for testing purposes"),
    ("test_slangdiag", "test diagnostics emission by the slang frontend"),
    ("test_slangexpr", "test expression evaluation within slang frontend"),
    ("timeest", "estimate timing"),
    ("torder", "print cells in topological order"),
    ("trace", "redirect command output to file"),
    ("tribuf", "infer tri-state buffers"),
    ("uniquify", "create unique copies of modules"),
    ("verific", "load Verilog and VHDL designs using Verific"),
    ("verilog_defaults", "set default options for read_verilog"),
    ("verilog_defines", "define and undefine verilog defines"),
    ("viz", "visualize data flow graph"),
    ("wbflip", "flip the whitebox attribute"),
    ("wrapcell", "wrap individual cells into new modules"),
    ("wreduce", "reduce the word size of operations if possible"),
    ("write_aiger", "write design to AIGER file"),
    ("write_aiger2", "(experimental) write design to AIGER file"),
    ("write_blif", "write design to BLIF file"),
    ("write_btor", "write design to BTOR file"),
    ("write_cxxrtl", "convert design to C++ RTL simulation"),
    ("write_edif", "write design to EDIF netlist file"),
    ("write_file", "write a text to a file"),
    ("write_firrtl", "write design to a FIRRTL file"),
    ("write_functional_cxx", "convert design to C++ using the functional backend"),
    ("write_functional_rosette", "Generate Rosette compatible Racket from Functional IR"),
    ("write_functional_smt2", "Generate SMT-LIB from Functional IR"),
    ("write_intersynth", "write design to InterSynth netlist file"),
    ("write_jny", "generate design metadata"),
    ("write_json", "write design to a JSON file"),
    ("write_rtlil", "write design to RTLIL file"),
    ("write_simplec", "convert design to simple C code"),
    ("write_smt2", "write design to SMT-LIBv2 file"),
    ("write_smv", "write design to SMV file"),
    ("write_spice", "write design to SPICE netlist file"),
    ("write_table", "write design as connectivity table"),
    ("write_verilog", "write design to Verilog file"),
    ("write_xaiger", "write design to XAIGER file"),
    ("write_xaiger2", "write module to XAIGER file"),
    ("xilinx_dffopt", "Xilinx: optimize FF control signal usage"),
    ("xilinx_dsp", "Xilinx: pack resources into DSPs"),
    ("xilinx_srl", "Xilinx shift register extraction"),
    ("xprop", "formal x propagation"),
    ("zinit", "add inverters so all FF are zero-initialized"),
];

// ---------------------------------------------------------------------------
// 批量收编第二批:六命令 help 全文常量(oracle `help <名>` 实跑转录,禁
// 手写;生成器=scripts/gen_help_batch2.sh,录制时点 2026-11-02)。消费面
// =cmd_error 全形 stdout 载体(§11.3-8);slice-1 起草期零消费(产品行为
// 零变更),dead_code 允许面随 slice-3..7 消费翻正(翻转 slice 同轮删除)。
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub const HELP_ASYNC2SYNC: &str = r#"
    async2sync [options] [selection]

This command replaces async FF inputs with sync circuits emulating the same
behavior for when the async signals are actually synchronized to the clock.

This pass assumes negative hold time for the async FF inputs. For example when
a reset deasserts with the clock edge, then the FF output will still drive the
reset value in the next cycle regardless of the data-in value at the time of
the clock edge.

    -nolower
        Do not automatically run 'chformal -lower' to lower $check cells.
"#;

#[allow(dead_code)]
pub const HELP_PMUXTREE: &str = r#"
    pmuxtree [selection]

This pass transforms $pmux cells to trees of $mux cells.
"#;

#[allow(dead_code)]
pub const HELP_DFFUNMAP: &str = r#"
    dffunmap [options] [selection]

This pass transforms FF types with clock enable and/or synchronous reset into
their base type (with neither clock enable nor sync reset) by emulating the
clock enable and synchronous reset with multiplexers on the cell input.

    -ce-only
        unmap only clock enables, leave synchronous resets alone.

    -srst-only
        unmap only synchronous resets, leave clock enables alone.
"#;

#[allow(dead_code)]
pub const HELP_SETATTR: &str = r#"
    setattr [ -mod ] [ -set name value | -unset name ]... [selection]

Set/unset the given attributes on the selected objects. String values must be
passed in double quotes (").

When called with -mod, this command will set and unset attributes on modules
instead of objects within modules.
"#;

#[allow(dead_code)]
pub const HELP_TORDER: &str = r#"
    torder [options] [selection]

This command prints the selected cells in topological order.

    -stop <cell_type> <cell_port>
        do not use the specified cell port in topological sorting

    -noautostop
        by default Q outputs of internal FF cells and memory read port outputs
        are not used in topological sorting. this option deactivates that.
"#;

#[allow(dead_code)]
pub const HELP_EDGETYPES: &str = r#"
    edgetypes [options] [selection]

This command lists all unique types of 'edges' found in the selection. An 'edge'
is a 4-tuple of source and sink cell type and port name.
"#;
