//! Bounded RFC 8259 JSON for requests and manifests.
//!
//! This application profile rejects duplicate decoded keys, BOM and unpaired
//! surrogates. Numbers retain their decimal text; no floating point is used.
//! Object keys are sorted. Typed job/schema validation is a separate next step.
use std::{collections::BTreeMap, fmt};

use crate::Work;

#[derive(Debug, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

/// Validated JSON number text. Equality compares its spelling, not numeric value.
#[derive(Debug, PartialEq, Eq)]
pub struct Number(String);

impl Number {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Local JSON ceilings, which callers may lower but not raise.
/// `token_bytes` bounds decoded strings/keys and raw number tokens.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub bytes: usize,
    pub depth: usize,
    pub token_bytes: usize,
    pub values: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: 8 * 1024 * 1024,
            depth: 64,
            token_bytes: 64 * 1024,
            values: 100_000,
        }
    }
}

impl Limits {
    fn validate(self) -> Result<(), Error> {
        let max = Self::default();
        if self.bytes > max.bytes
            || self.depth > max.depth
            || self.token_bytes > max.token_bytes
            || self.values > max.values
        {
            return Err(error(0, ErrorKind::Limit));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Syntax(&'static str),
    DuplicateKey,
    Limit,
    Allocation,
}

/// Zero-based UTF-8 byte offset. Messages never echo input content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error {
    pub offset: usize,
    pub kind: ErrorKind,
}

impl Error {
    pub fn code(self) -> &'static str {
        match self.kind {
            ErrorKind::Syntax(_) | ErrorKind::DuplicateKey => "E_JSON",
            ErrorKind::Limit => "E_LIMIT",
            ErrorKind::Allocation => "E_ALLOC",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.kind {
            ErrorKind::Syntax(message) => message,
            ErrorKind::DuplicateKey => "Duplicate decoded object key",
            ErrorKind::Limit => "JSON or work limit exceeded",
            ErrorKind::Allocation => "Unable to reserve JSON memory",
        };
        write!(f, "{message} at byte {}", self.offset)
    }
}
impl std::error::Error for Error {}

fn error(offset: usize, kind: ErrorKind) -> Error {
    Error { offset, kind }
}

/// Parse exactly one UTF-8 document, charging input bytes plus one per value.
/// Owned data is bounded by input, token and value ceilings. String/vector
/// reservations are fallible; std map allocation can still abort on OOM.
/// This is not the future aggregate job-memory accountant.
pub fn parse(input: &[u8], limits: Limits, work: &mut Work) -> Result<Value, Error> {
    limits.validate()?;
    if input.len() > limits.bytes {
        return Err(error(0, ErrorKind::Limit));
    }
    work.charge(input.len() as u64)
        .map_err(|_| error(0, ErrorKind::Limit))?;
    let text = std::str::from_utf8(input)
        .map_err(|e| error(e.valid_up_to(), ErrorKind::Syntax("Invalid UTF-8")))?;
    let mut parser = Parser {
        text,
        pos: 0,
        values: 0,
        limits,
        work,
    };
    let value = parser.value(0)?;
    parser.whitespace();
    if parser.pos != text.len() {
        return Err(parser.syntax("Unexpected trailing data"));
    }
    Ok(value)
}

struct Parser<'a, 'w> {
    text: &'a str,
    pos: usize,
    values: usize,
    limits: Limits,
    work: &'w mut Work,
}

impl Parser<'_, '_> {
    fn syntax(&self, message: &'static str) -> Error {
        error(self.pos, ErrorKind::Syntax(message))
    }
    fn byte(&self) -> Option<u8> {
        self.text.as_bytes().get(self.pos).copied()
    }
    fn take(&mut self, byte: u8) -> bool {
        if self.byte() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, byte: u8) -> Result<(), Error> {
        if self.take(byte) {
            Ok(())
        } else {
            Err(self.syntax("Expected JSON delimiter"))
        }
    }
    fn whitespace(&mut self) {
        while matches!(self.byte(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.pos += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, Error> {
        self.whitespace();
        if self.values == self.limits.values {
            return Err(error(self.pos, ErrorKind::Limit));
        }
        self.work
            .charge(1)
            .map_err(|_| error(self.pos, ErrorKind::Limit))?;
        self.values += 1;
        match self.byte() {
            Some(b'n') => {
                self.literal("null")?;
                Ok(Value::Null)
            }
            Some(b't') => {
                self.literal("true")?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.literal("false")?;
                Ok(Value::Bool(false))
            }
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b'-' | b'0'..=b'9') => Ok(Value::Number(self.number()?)),
            Some(b'[' | b'{') if depth >= self.limits.depth => {
                Err(error(self.pos, ErrorKind::Limit))
            }
            Some(b'[') => self.array(depth + 1),
            Some(b'{') => self.object(depth + 1),
            _ => Err(self.syntax("Expected JSON value")),
        }
    }

    fn literal(&mut self, text: &str) -> Result<(), Error> {
        if !self.text[self.pos..].starts_with(text) {
            return Err(self.syntax("Invalid JSON literal"));
        }
        self.pos += text.len();
        Ok(())
    }

    fn array(&mut self, depth: usize) -> Result<Value, Error> {
        self.pos += 1;
        self.whitespace();
        let mut array = Vec::new();
        if self.take(b']') {
            return Ok(Value::Array(array));
        }
        loop {
            let value = self.value(depth)?;
            array
                .try_reserve(1)
                .map_err(|_| error(self.pos, ErrorKind::Allocation))?;
            array.push(value);
            self.whitespace();
            if self.take(b']') {
                return Ok(Value::Array(array));
            }
            self.expect(b',')?;
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, Error> {
        self.pos += 1;
        self.whitespace();
        let mut object = BTreeMap::new();
        if self.take(b'}') {
            return Ok(Value::Object(object));
        }
        loop {
            self.whitespace();
            let key_offset = self.pos;
            let key = self.string()?;
            if object.contains_key(&key) {
                return Err(error(key_offset, ErrorKind::DuplicateKey));
            }
            self.whitespace();
            self.expect(b':')?;
            let value = self.value(depth)?;
            object.insert(key, value);
            self.whitespace();
            if self.take(b'}') {
                return Ok(Value::Object(object));
            }
            self.expect(b',')?;
        }
    }

    fn number(&mut self) -> Result<Number, Error> {
        let start = self.pos;
        self.take(b'-');
        match self.byte() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return Err(self.syntax("Expected number digit")),
        }
        if self.take(b'.') {
            if !matches!(self.byte(), Some(b'0'..=b'9')) {
                return Err(self.syntax("Expected fractional digit"));
            }
            self.digits();
        }
        if self.take(b'e') || self.take(b'E') {
            if !self.take(b'+') {
                self.take(b'-');
            }
            if !matches!(self.byte(), Some(b'0'..=b'9')) {
                return Err(self.syntax("Expected exponent digit"));
            }
            self.digits();
        }
        let mut number = String::new();
        append(
            &mut number,
            &self.text[start..self.pos],
            self.limits.token_bytes,
            start,
        )?;
        Ok(Number(number))
    }

    fn digits(&mut self) {
        while matches!(self.byte(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
    }

    fn string(&mut self) -> Result<String, Error> {
        self.expect(b'"')?;
        let mut out = String::new();
        let mut start = self.pos;
        loop {
            match self.byte() {
                Some(b'"') => {
                    append(
                        &mut out,
                        &self.text[start..self.pos],
                        self.limits.token_bytes,
                        self.pos,
                    )?;
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    append(
                        &mut out,
                        &self.text[start..self.pos],
                        self.limits.token_bytes,
                        self.pos,
                    )?;
                    self.pos += 1;
                    let ch = self.escape()?;
                    let mut bytes = [0; 4];
                    append(
                        &mut out,
                        ch.encode_utf8(&mut bytes),
                        self.limits.token_bytes,
                        self.pos,
                    )?;
                    start = self.pos;
                }
                Some(0..=0x1f) => return Err(self.syntax("Unescaped string control character")),
                Some(_) => {
                    self.pos += 1;
                    if out.len() + self.pos - start > self.limits.token_bytes {
                        return Err(error(self.pos, ErrorKind::Limit));
                    }
                }
                None => return Err(self.syntax("Unterminated string")),
            }
        }
    }

    fn escape(&mut self) -> Result<char, Error> {
        let byte = self.byte().ok_or_else(|| self.syntax("Truncated escape"))?;
        self.pos += 1;
        match byte {
            b'"' => Ok('"'),
            b'\\' => Ok('\\'),
            b'/' => Ok('/'),
            b'b' => Ok('\u{8}'),
            b'f' => Ok('\u{c}'),
            b'n' => Ok('\n'),
            b'r' => Ok('\r'),
            b't' => Ok('\t'),
            b'u' => {
                let high = self.hex4()?;
                let scalar = if (0xd800..=0xdbff).contains(&high) {
                    self.expect(b'\\')?;
                    self.expect(b'u')?;
                    let low = self.hex4()?;
                    if !(0xdc00..=0xdfff).contains(&low) {
                        return Err(self.syntax("Expected low surrogate"));
                    }
                    0x10000 + ((high - 0xd800) << 10) + low - 0xdc00
                } else {
                    high
                };
                char::from_u32(scalar).ok_or_else(|| self.syntax("Unpaired surrogate"))
            }
            _ => Err(self.syntax("Invalid string escape")),
        }
    }

    fn hex4(&mut self) -> Result<u32, Error> {
        let mut value = 0;
        for _ in 0..4 {
            let digit = match self.byte() {
                Some(b'0'..=b'9') => u32::from(self.byte().unwrap() - b'0'),
                Some(b'a'..=b'f') => u32::from(self.byte().unwrap() - b'a' + 10),
                Some(b'A'..=b'F') => u32::from(self.byte().unwrap() - b'A' + 10),
                _ => return Err(self.syntax("Expected four hexadecimal digits")),
            };
            value = value * 16 + digit;
            self.pos += 1;
        }
        Ok(value)
    }
}

fn append(out: &mut String, text: &str, limit: usize, offset: usize) -> Result<(), Error> {
    if text.len() > limit.saturating_sub(out.len()) {
        return Err(error(offset, ErrorKind::Limit));
    }
    out.try_reserve(text.len())
        .map_err(|_| error(offset, ErrorKind::Allocation))?;
    out.push_str(text);
    Ok(())
}
