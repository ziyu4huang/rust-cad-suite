//! 最小表達式 printer(vl-syntax AST → 源碼渲染文本;§4.2)。
//!
//! 還原運算符/括號優先級;**不改寫、不求值**(禁區;求值是 vl-spec/vl-sim
//! 軌)。渲染文本存進 check 欄位與 statement,可檢驗性=「文本可對回源碼 +
//! form 欄位可機械判定」。

use vl_syntax::ast::{self, Base, BinOp, Expr, UnOp};

/// (precedence, symbol); precedence 小=綁定弱(先加括號)。
fn bin_op_info(op: BinOp) -> (u8, &'static str) {
    match op {
        BinOp::LogOr => (2, "||"),
        BinOp::LogAnd => (3, "&&"),
        BinOp::BitOr => (4, "|"),
        BinOp::BitXor => (5, "^"),
        BinOp::BitAnd => (6, "&"),
        BinOp::Eq => (7, "=="),
        BinOp::Ne => (7, "!="),
        BinOp::EqCase => (7, "==="),
        BinOp::NeCase => (7, "!=="),
        BinOp::Lt => (8, "<"),
        BinOp::Le => (8, "<="),
        BinOp::Gt => (8, ">"),
        BinOp::Ge => (8, ">="),
        BinOp::Shl => (9, "<<"),
        BinOp::Shr => (9, ">>"),
        BinOp::AShl => (9, "<<<"),
        BinOp::AShr => (9, ">>>"),
        BinOp::Add => (10, "+"),
        BinOp::Sub => (10, "-"),
        BinOp::Mul => (11, "*"),
        BinOp::Div => (11, "/"),
        BinOp::Mod => (11, "%"),
        BinOp::Pow => (11, "**"),
    }
}

fn un_op_sym(op: UnOp) -> &'static str {
    match op {
        UnOp::Not => "!",
        UnOp::BitNot => "~",
        UnOp::Plus => "+",
        UnOp::Minus => "-",
        UnOp::And => "&",
        UnOp::Nand => "~&",
        UnOp::Or => "|",
        UnOp::Nor => "~|",
        UnOp::Xor => "^",
        UnOp::Xnor => "~^",
    }
}

fn num_lit(n: &ast::NumLit) -> String {
    let mut s = String::new();
    if let Some(w) = n.width {
        s.push_str(&w.to_string());
    }
    s.push('\'');
    if n.signed {
        s.push('s');
    }
    s.push(match n.base {
        Base::Dec => 'd',
        Base::Bin => 'b',
        Base::Oct => 'o',
        Base::Hex => 'h',
    });
    s.push_str(&n.digits);
    s
}

/// 渲染 `e`;`min_prec` 以下的子式加括號。
pub fn render(e: &Expr) -> String {
    render_min(e, 0)
}

fn render_min(e: &Expr, min_prec: u8) -> String {
    let (s, prec) = render_prec(e);
    if prec < min_prec {
        format!("({s})")
    } else {
        s
    }
}

/// 返回 (text, self precedence);postfix/primary 返回 u8::MAX。
fn render_prec(e: &Expr) -> (String, u8) {
    match e {
        Expr::Ident { name, .. } => (name.clone(), u8::MAX),
        Expr::HierIdent { parts, .. } => (parts.join("."), u8::MAX),
        Expr::HierDot { base, name, .. } => {
            (format!("{}.{}", render_min(base, u8::MAX), name), u8::MAX)
        }
        Expr::NumLit(n) => (num_lit(n), u8::MAX),
        Expr::RealLit { text, .. } => (text.clone(), u8::MAX),
        Expr::StrLit { value, .. } => (format!("\"{value}\""), u8::MAX),
        Expr::Unary { op, expr, .. } => {
            let sym = un_op_sym(*op);
            // 一元緊跟其操作數(同級鏈如 ~&x 不會出現二層,保險起見給 12)。
            (format!("{sym}{}", render_min(expr, 12)), 12)
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            let (p, sym) = bin_op_info(*op);
            (
                format!("{} {sym} {}", render_min(lhs, p), render_min(rhs, p + 1)),
                p,
            )
        }
        Expr::Ternary { cond, then, els, .. } => (
            format!(
                "{} ? {} : {}",
                render_min(cond, 2),
                render_min(then, 1),
                render_min(els, 1)
            ),
            1,
        ),
        Expr::Concat { parts, .. } => {
            let inner: Vec<String> = parts.iter().map(|p| render_min(p, 0)).collect();
            (format!("{{{}}}", inner.join(", ")), u8::MAX)
        }
        Expr::Repeat { count, value, .. } => (
            format!("{}{{{}}}", render_min(count, u8::MAX), render_min(value, 0)),
            u8::MAX,
        ),
        Expr::Index { base, index, .. } => (
            format!(
                "{}[{}]",
                render_min(base, u8::MAX),
                render_min(index, 0)
            ),
            u8::MAX,
        ),
        Expr::PartSelect { base, msb, lsb, .. } => (
            format!(
                "{}[{}:{}]",
                render_min(base, u8::MAX),
                render_min(msb, 0),
                render_min(lsb, 0)
            ),
            u8::MAX,
        ),
        Expr::IndexedPart { base, offset, width, plus, .. } => (
            format!(
                "{}[{} {}: {}]",
                render_min(base, u8::MAX),
                render_min(offset, 0),
                if *plus { "+:" } else { "-:" },
                render_min(width, 0)
            ),
            u8::MAX,
        ),
        Expr::Call { func, args, .. } => {
            let inner: Vec<String> = args.iter().map(|a| render_min(a, 0)).collect();
            (format!("{func}({})", inner.join(", ")), u8::MAX)
        }
        Expr::Delay3 { min, typ, max, .. } => (
            format!(
                "({}:{}:{})",
                render_min(min, 0),
                render_min(typ, 0),
                render_min(max, 0)
            ),
            u8::MAX,
        ),
    }
}

/// rhs 是否含任何 Binary 節點(arithmetic 判據,R1-3 放寬:算術+位運算/邏輯)。
pub fn has_binary(e: &Expr) -> bool {
    match e {
        Expr::Binary { .. } => true,
        Expr::Unary { expr, .. } => has_binary(expr),
        Expr::Ternary { cond, then, els, .. } => {
            has_binary(cond) || has_binary(then) || has_binary(els)
        }
        Expr::Concat { parts, .. } => parts.iter().any(has_binary),
        Expr::Repeat { count, value, .. } => has_binary(count) || has_binary(value),
        Expr::Index { base, index, .. } => has_binary(base) || has_binary(index),
        Expr::PartSelect { base, msb, lsb, .. } => {
            has_binary(base) || has_binary(msb) || has_binary(lsb)
        }
        Expr::IndexedPart { base, offset, width, .. } => {
            has_binary(base) || has_binary(offset) || has_binary(width)
        }
        Expr::HierDot { base, .. } => has_binary(base),
        Expr::Call { args, .. } => args.iter().any(has_binary),
        _ => false,
    }
}

/// lvalue 的基底標識符名(Ident/HierIdent 首段;Index/PartSelect/IndexedPart
/// /HierDot 遞進 base;Concat=None——拼接賦值不成單一 subject)。
pub fn lvalue_base(e: &Expr) -> Option<String> {
    match e {
        Expr::Ident { name, .. } => Some(name.clone()),
        Expr::HierIdent { parts, .. } => parts.first().cloned(),
        Expr::HierDot { base, .. } => lvalue_base(base),
        Expr::Index { base, .. }
        | Expr::PartSelect { base, .. }
        | Expr::IndexedPart { base, .. } => lvalue_base(base),
        _ => None,
    }
}

/// 遍歷子式(含自身);`f` 返回 true 即短路。
pub fn walk_expr(e: &Expr, f: &mut impl FnMut(&Expr) -> bool) -> bool {
    if f(e) {
        return true;
    }
    match e {
        Expr::Unary { expr, .. } => walk_expr(expr, f),
        Expr::Binary { lhs, rhs, .. } => walk_expr(lhs, f) || walk_expr(rhs, f),
        Expr::Ternary { cond, then, els, .. } => {
            walk_expr(cond, f) || walk_expr(then, f) || walk_expr(els, f)
        }
        Expr::Concat { parts, .. } => parts.iter().any(|p| walk_expr(p, f)),
        Expr::Repeat { count, value, .. } => walk_expr(count, f) || walk_expr(value, f),
        Expr::Index { base, index, .. } => walk_expr(base, f) || walk_expr(index, f),
        Expr::PartSelect { base, msb, lsb, .. } => {
            walk_expr(base, f) || walk_expr(msb, f) || walk_expr(lsb, f)
        }
        Expr::IndexedPart { base, offset, width, .. } => {
            walk_expr(base, f) || walk_expr(offset, f) || walk_expr(width, f)
        }
        Expr::HierDot { base, .. } => walk_expr(base, f),
        Expr::Call { args, .. } => args.iter().any(|a| walk_expr(a, f)),
        _ => false,
    }
}
