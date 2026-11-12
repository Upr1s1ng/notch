use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum JsonVal {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<JsonVal>),
    Obj(BTreeMap<String, JsonVal>),
}

impl JsonVal {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_obj(&self) -> Option<&BTreeMap<String, JsonVal>> {
        match self {
            Self::Obj(m) => Some(m),
            _ => None,
        }
    }
}

pub fn escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn to_string_pretty(val: &JsonVal) -> String {
    let mut out = String::new();
    write_pretty(val, &mut out, 0);
    out.push('\n');
    out
}

fn write_pretty(val: &JsonVal, out: &mut String, indent: usize) {
    match val {
        JsonVal::Null => out.push_str("null"),
        JsonVal::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        JsonVal::Num(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                out.push_str(&format!("{}", *n as i64));
            } else {
                out.push_str(&format!("{n}"));
            }
        }
        JsonVal::Str(s) => out.push_str(&escape_str(s)),
        JsonVal::Arr(arr) => {
            if arr.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (i, item) in arr.iter().enumerate() {
                out.push_str(&"  ".repeat(indent + 1));
                write_pretty(item, out, indent + 1);
                if i + 1 < arr.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&"  ".repeat(indent));
            out.push(']');
        }
        JsonVal::Obj(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{\n");
            let len = map.len();
            for (i, (k, v)) in map.iter().enumerate() {
                out.push_str(&"  ".repeat(indent + 1));
                out.push_str(&escape_str(k));
                out.push_str(": ");
                write_pretty(v, out, indent + 1);
                if i + 1 < len {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&"  ".repeat(indent));
            out.push('}');
        }
    }
}

pub fn parse(src: &str) -> Result<JsonVal, String> {
    let mut p = Parser {
        b: src.as_bytes(),
        i: 0,
    };
    p.ws();
    let v = p.val()?;
    p.ws();
    if p.i != p.b.len() {
        return Err("trailing characters".into());
    }
    Ok(v)
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn val(&mut self) -> Result<JsonVal, String> {
        self.ws();
        match self.peek() {
            Some(b'n') => self.lit(b"null", JsonVal::Null),
            Some(b't') => self.lit(b"true", JsonVal::Bool(true)),
            Some(b'f') => self.lit(b"false", JsonVal::Bool(false)),
            Some(b'"') => Ok(JsonVal::Str(self.str_lit()?)),
            Some(b'[') => self.arr(),
            Some(b'{') => self.obj(),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.num(),
            _ => Err("unexpected token".into()),
        }
    }

    fn lit(&mut self, kw: &[u8], val: JsonVal) -> Result<JsonVal, String> {
        if self.b[self.i..].starts_with(kw) {
            self.i += kw.len();
            Ok(val)
        } else {
            Err("invalid literal".into())
        }
    }

    fn str_lit(&mut self) -> Result<String, String> {
        self.i += 1;
        let mut out = String::new();
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.i += 1;
            if c == b'"' {
                return Ok(out);
            }
            if c == b'\\' {
                let esc = *self.b.get(self.i).ok_or("eof in escape")?;
                self.i += 1;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    _ => out.push(esc as char),
                }
            } else {
                out.push(c as char);
            }
        }
        Err("unterminated string".into())
    }

    fn num(&mut self) -> Result<JsonVal, String> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-') {
                self.i += 1;
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&self.b[start..self.i]).map_err(|_| "utf8")?;
        s.parse::<f64>()
            .map(JsonVal::Num)
            .map_err(|_| "bad num".into())
    }

    fn arr(&mut self) -> Result<JsonVal, String> {
        self.i += 1;
        let mut out = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(JsonVal::Arr(out));
        }
        loop {
            out.push(self.val()?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(JsonVal::Arr(out));
                }
                _ => return Err("expected , or ]".into()),
            }
        }
    }

    fn obj(&mut self) -> Result<JsonVal, String> {
        self.i += 1;
        let mut out = BTreeMap::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(JsonVal::Obj(out));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err("expected key".into());
            }
            let k = self.str_lit()?;
            self.ws();
            if self.peek() != Some(b':') {
                return Err("expected :".into());
            }
            self.i += 1;
            let v = self.val()?;
            out.insert(k, v);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(JsonVal::Obj(out));
                }
                _ => return Err("expected , or }".into()),
            }
        }
    }
}
