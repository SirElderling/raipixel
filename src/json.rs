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
    /// Convert exactly, accepting integral decimal/exponent forms without rounding.
    pub fn as_i64(&self) -> Option<i64> {
        let magnitude = i128::from(self.magnitude()?);
        let signed = if self.0.starts_with('-') {
            -magnitude
        } else {
            magnitude
        };
        i64::try_from(signed).ok()
    }

    pub fn as_u64(&self) -> Option<u64> {
        let magnitude = self.magnitude()?;
        if self.0.starts_with('-') && magnitude != 0 {
            None
        } else {
            Some(magnitude)
        }
    }

    fn magnitude(&self) -> Option<u64> {
        let unsigned = self.0.strip_prefix('-').unwrap_or(&self.0);
        let (mantissa, exponent) = unsigned.split_once(['e', 'E']).unwrap_or((unsigned, "0"));
        let digits = mantissa
            .bytes()
            .filter(|b| b.is_ascii_digit())
            .skip_while(|b| *b == b'0');
        let significant = digits.clone().count();
        if significant == 0 {
            return Some(0);
        }
        let fraction = mantissa
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len());
        // Saturation is sufficient: a nonzero bounded token cannot cancel an
        // exponent larger than i64. No exponent-sized allocation or traversal.
        let exponent_negative = exponent.starts_with('-');
        let exponent_digits = exponent.trim_start_matches(['+', '-']);
        let mut scale = 0i64;
        for digit in exponent_digits.bytes() {
            scale = scale
                .saturating_mul(10)
                .saturating_add(i64::from(digit - b'0'));
        }
        if exponent_negative {
            scale = -scale;
        }
        scale = scale.saturating_sub(fraction as i64);
        let mut keep = significant;
        if scale < 0 {
            let trailing = mantissa
                .bytes()
                .rev()
                .filter(|b| b.is_ascii_digit())
                .take_while(|b| *b == b'0')
                .count();
            let remove = scale.unsigned_abs();
            if remove > trailing as u64 {
                return None;
            }
            keep -= remove as usize;
            scale = 0;
        }
        if keep as u64 + scale as u64 > 20 {
            return None;
        }
        let mut value = 0u64;
        for digit in digits.take(keep) {
            value = value
                .checked_mul(10)?
                .checked_add(u64::from(digit - b'0'))?;
        }
        for _ in 0..scale {
            value = value.checked_mul(10)?;
        }
        Some(value)
    }
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
                Some(byte @ b'0'..=b'9') => u32::from(byte - b'0'),
                Some(byte @ b'a'..=b'f') => u32::from(byte - b'a' + 10),
                Some(byte @ b'A'..=b'F') => u32::from(byte - b'A' + 10),
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

/// Emit compact UTF-8 JSON with sorted object keys and a terminal newline.
/// Number spellings are retained; this is not a native manifest/schema writer.
/// Charges one unit per visited value plus actual output bytes. Errors discard
/// the private output buffer, but may consume already charged work.
pub fn emit(value: &Value, limits: Limits, work: &mut Work) -> Result<Vec<u8>, Error> {
    limits.validate()?;
    let mut writer = Writer {
        out: Vec::new(),
        values: 0,
        limits,
        work,
    };
    writer.value(value, 0)?;
    writer.push(b"\n")?;
    Ok(writer.out)
}

struct Writer<'w> {
    out: Vec<u8>,
    values: usize,
    limits: Limits,
    work: &'w mut Work,
}

impl Writer<'_> {
    // Emission errors identify the byte position reached in the private buffer.
    fn limit(&self) -> Error {
        error(self.out.len(), ErrorKind::Limit)
    }

    fn push(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() > self.limits.bytes.saturating_sub(self.out.len()) {
            return Err(self.limit());
        }
        self.work
            .charge(bytes.len() as u64)
            .map_err(|_| self.limit())?;
        self.out
            .try_reserve(bytes.len())
            .map_err(|_| error(self.out.len(), ErrorKind::Allocation))?;
        self.out.extend_from_slice(bytes);
        Ok(())
    }

    fn value(&mut self, value: &Value, depth: usize) -> Result<(), Error> {
        if self.values == self.limits.values {
            return Err(self.limit());
        }
        self.work.charge(1).map_err(|_| self.limit())?;
        self.values += 1;
        match value {
            Value::Null => self.push(b"null"),
            Value::Bool(true) => self.push(b"true"),
            Value::Bool(false) => self.push(b"false"),
            Value::Number(number) => {
                if number.0.len() > self.limits.token_bytes {
                    return Err(self.limit());
                }
                self.push(number.0.as_bytes())
            }
            Value::String(text) => self.string(text),
            Value::Array(_) | Value::Object(_) if depth >= self.limits.depth => Err(self.limit()),
            Value::Array(values) => {
                self.push(b"[")?;
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        self.push(b",")?;
                    }
                    self.value(value, depth + 1)?;
                }
                self.push(b"]")
            }
            Value::Object(values) => {
                self.push(b"{")?;
                for (index, (key, value)) in values.iter().enumerate() {
                    if index != 0 {
                        self.push(b",")?;
                    }
                    self.string(key)?;
                    self.push(b":")?;
                    self.value(value, depth + 1)?;
                }
                self.push(b"}")
            }
        }
    }

    fn string(&mut self, text: &str) -> Result<(), Error> {
        if text.len() > self.limits.token_bytes {
            return Err(self.limit());
        }
        self.push(b"\"")?;
        for ch in text.chars() {
            match ch {
                '"' => self.push(b"\\\"")?,
                '\\' => self.push(b"\\\\")?,
                '\u{8}' => self.push(b"\\b")?,
                '\u{c}' => self.push(b"\\f")?,
                '\n' => self.push(b"\\n")?,
                '\r' => self.push(b"\\r")?,
                '\t' => self.push(b"\\t")?,
                '\0'..='\u{1f}' => {
                    const HEX: &[u8; 16] = b"0123456789ABCDEF";
                    let code = ch as usize;
                    self.push(&[b'\\', b'u', b'0', b'0', HEX[code >> 4], HEX[code & 15]])?;
                }
                _ => {
                    let mut bytes = [0; 4];
                    self.push(ch.encode_utf8(&mut bytes).as_bytes())?;
                }
            }
        }
        self.push(b"\"")
    }
}
