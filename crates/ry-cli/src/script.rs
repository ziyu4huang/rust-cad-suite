//! `-p` 腳本切分契約(RYOSYS-MVP §0-11/§1.2)。
//!
//! 逐字移植 kernel/register.cc `Pass::call(design, string)` 的詞法循環
//! + kernel/io.cc `next_token(text, " \t\r\n", true)`:
//! * token 以空白(` \t\r\n`)定界;
//! * **僅尾字元為 `;` 時切分**:剝掉全部尾隨 `;`(計數),非空殼入 args
//!   並執行;`;;`→追加 `clean`、`;;;`→追加 `clean -purge`(僅這兩檔);
//! * `#` 開頭 token=註釋:吃到下一個 `\r`/`\n`(換行本身保留,隨後由
//!   換行邊界規則觸發一次空執行);
//! * 換行邊界:剩餘串中**第一個非空白字元**是 `\r`/`\n` → 執行累積 args;
//! * 引號 token:`"` 開頭時吃到「後跟分隔符或 EOF 的 `"`」,引號保留在
//!   token 內;`";`+分隔符形(token 尾帶一個 `;`)照常進入尾分號分支;
//! * 首 token 為 `!` 開頭=shell 逸出(M0 硬錯,Y0)。

/// 切分結果:每項一條命令的 argv。
pub fn split_script(script: &str) -> Result<Vec<Vec<String>>, String> {
    let mut commands: Vec<Vec<String>> = Vec::new();
    let mut buf = script.to_string();
    let mut args: Vec<String> = Vec::new();

    let mut tok = next_token(&mut buf);
    if tok.is_empty() {
        return Ok(commands);
    }
    if tok.starts_with('!') {
        return Err("Y0".to_string()); // shell 逸出:M0 不做(§1.3)
    }

    while !tok.is_empty() {
        if tok.starts_with('#') {
            // 註釋:吃到 \r 或 \n(含)之前的內容丟棄,換行保留。
            let stop = buf.find(['\r', '\n']).unwrap_or(buf.len());
            buf = buf[stop..].to_string();
        } else if tok.ends_with(';') {
            let mut num_semi = 0usize;
            let mut t = tok.clone();
            while t.ends_with(';') {
                t.pop();
                num_semi += 1;
            }
            if !t.is_empty() {
                args.push(t);
            }
            if !args.is_empty() {
                commands.push(std::mem::take(&mut args));
            }
            if num_semi == 2 {
                for c in split_script("clean")? {
                    commands.push(c);
                }
            }
            if num_semi == 3 {
                for c in split_script("clean -purge")? {
                    commands.push(c);
                }
            }
        } else {
            args.push(tok.clone());
        }
        // 換行邊界:剩餘的第一個非 ' '/'\t' 字元是 \r/\n → 執行累積 args。
        let found_nl = buf
            .chars()
            .find(|c| *c != ' ' && *c != '\t')
            .map(|c| c == '\r' || c == '\n')
            .unwrap_or(false);
        if found_nl && !args.is_empty() {
            commands.push(std::mem::take(&mut args));
        }
        tok = next_token(&mut buf);
    }
    if !args.is_empty() {
        commands.push(args);
    }
    Ok(commands)
}

/// kernel/io.cc next_token(text, " \t\r\n", long_strings=true) 的等價:
/// 引號 token 保留引號;`";`+分隔符形在尾部補一個 `;`。
fn next_token(text: &mut String) -> String {
    let sep: &[char] = &[' ', '\t', '\r', '\n'];
    let bytes = text.as_str();
    let pos_begin = match bytes.find(|c: char| !sep.contains(&c)) {
        Some(p) => p,
        None => {
            *text = String::new();
            return String::new();
        }
    };
    let rest = &bytes[pos_begin..];
    if rest.starts_with('"') {
        let chars: Vec<char> = rest.chars().collect();
        let mut i = 1usize;
        while i < chars.len() {
            if chars[i] == '"'
                && (i + 1 == chars.len() || sep.contains(&chars[i + 1]))
            {
                let token: String = chars[..=i].iter().collect();
                *text = chars[i + 1..].iter().collect();
                return token;
            }
            if chars[i] == '"'
                && i + 1 < chars.len()
                && chars[i + 1] == ';'
                && (i + 2 == chars.len() || sep.contains(&chars[i + 2]))
            {
                let mut token: String = chars[..=i].iter().collect();
                token.push(';');
                *text = chars[i + 2..].iter().collect();
                return token;
            }
            i += 1;
        }
    }
    let pos_end = rest.find(|c: char| sep.contains(&c)).map(|p| pos_begin + p);
    match pos_end {
        Some(e) => {
            let token = bytes[pos_begin..e].to_string();
            *text = bytes[e..].to_string();
            token
        }
        None => {
            let token = rest.to_string();
            *text = String::new();
            token
        }
    }
}
