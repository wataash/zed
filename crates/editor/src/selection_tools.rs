// SPDX-FileCopyrightText: Copyright (c) 2026 Wataru Ashihara <wataash0607@gmail.com>
// SPDX-License-Identifier: Apache-2.0
use std::collections::HashSet;

pub fn unique_lines(text: &str) -> String {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let trailing = text.ends_with(eol);
    let mut seen = HashSet::new();
    let mut result = text
        .strip_suffix(eol)
        .unwrap_or(text)
        .split(eol)
        .filter(|line| seen.insert(*line))
        .collect::<Vec<_>>()
        .join(eol);
    if trailing {
        result.push_str(eol);
    }
    result
}

pub fn code_block(text: &str, row: usize) -> Option<String> {
    let lines: Vec<_> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let mut opening: Option<(usize, char, usize)> = None;
    for (i, line) in lines.iter().enumerate() {
        let line = line.trim_start();
        let ch = line.chars().next().unwrap_or(' ');
        let count = line.chars().take_while(|c| *c == ch).count();
        if ch != '`' && ch != '~' || count < 3 {
            continue;
        }
        let rest = &line[count..];
        if let Some((start, fence, length)) = opening {
            if ch == fence && count >= length && rest.trim().is_empty() {
                if row >= start && row < i {
                    return Some(lines[start + 1..i].join("\n"));
                }
                opening = None;
            }
        } else if ch != '`' || !rest.contains('`') {
            opening = Some((i, ch, count));
        }
    }
    None
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
    depth: usize,
    significant: usize,
}
impl Parser<'_> {
    fn skip(&mut self) {
        while let Some(c) = self.input[self.pos..].chars().next() {
            if !c.is_whitespace() && c != '\u{feff}' {
                break;
            }
            self.pos += c.len_utf8();
        }
    }
    fn take(&mut self, token: &str) -> bool {
        self.skip();
        if self.input[self.pos..].starts_with(token) {
            self.pos += token.len();
            true
        } else {
            false
        }
    }
    fn additive(&mut self) -> Option<f64> {
        let mut v = self.multiply()?;
        loop {
            if self.take("+") {
                v += self.multiply()?;
            } else if self.take("-") {
                v -= self.multiply()?;
            } else {
                return Some(v);
            }
        }
    }
    fn multiply(&mut self) -> Option<f64> {
        let mut v = self.unary()?;
        loop {
            self.skip();
            if self.input[self.pos..].starts_with("**") {
                return Some(v);
            }
            if self.take("*") || self.take("x") {
                v *= self.unary()?;
            } else if self.take("/") {
                let rhs = self.unary()?;
                if rhs == 0.0 {
                    return None;
                }
                v /= rhs;
            } else {
                return Some(v);
            }
        }
    }
    fn unary(&mut self) -> Option<f64> {
        if self.depth >= 128 {
            return None;
        }
        self.depth += 1;
        let result = if self.take("+") {
            self.unary()
        } else if self.take("-") {
            self.unary().map(|v| -v)
        } else {
            self.power()
        };
        self.depth -= 1;
        result
    }
    fn power(&mut self) -> Option<f64> {
        let base = self.primary()?;
        if self.take("**") || self.take("^") {
            Some(base.powf(self.unary()?))
        } else {
            Some(base)
        }
    }
    fn primary(&mut self) -> Option<f64> {
        if self.take("(") {
            let v = self.additive()?;
            return self.take(")").then_some(v);
        }
        self.skip();
        let start = self.pos;
        let bytes = self.input.as_bytes();
        while self.pos < bytes.len()
            && (bytes[self.pos].is_ascii_digit() || bytes[self.pos] == b',')
        {
            self.pos += 1;
        }
        let integer = &self.input[start..self.pos];
        let groups: Vec<_> = integer.split(',').collect();
        if groups[0].is_empty()
            || groups.len() > 1 && (groups[0].len() > 3 || groups[1..].iter().any(|g| g.len() != 3))
        {
            return None;
        }
        if self.pos < bytes.len() && bytes[self.pos] == b'.' {
            self.pos += 1;
            let decimal = self.pos;
            while self.pos < bytes.len() && bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
            if decimal == self.pos {
                return None;
            }
        }
        let token = self.input[start..self.pos].replace(',', "");
        let digits = token.replace('.', "");
        let significant = digits.trim_start_matches('0').len();
        self.significant = self.significant.max(if significant == 0 {
            token
                .split_once('.')
                .map_or(1, |(_, frac)| frac.len().max(1))
        } else {
            significant
        });
        token.parse().ok()
    }
}

fn group(text: String, expression: &str) -> String {
    if !expression.contains(',') || text.contains('e') {
        return text;
    }
    let (sign, rest) = if let Some(rest) = text.strip_prefix('-') {
        ("-", rest)
    } else {
        ("", text.as_str())
    };
    let (integer, fraction) = rest.split_once('.').unwrap_or((rest, ""));
    let mut result = sign.to_owned();
    for (i, ch) in integer.chars().enumerate() {
        if i > 0 && (integer.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    if rest.contains('.') {
        result.push('.');
        result.push_str(fraction);
    }
    result
}

// Round the shortest decimal representation, with ties away from zero.
// String arithmetic avoids binary tie artifacts such as 1.005 rounding to 1.00.
fn round_decimal(value: f64, places: i32) -> String {
    let decimal = value.abs().to_string();
    let (integer, fraction) = decimal.split_once('.').unwrap_or((&decimal, ""));
    let mut digits = format!("{integer}{fraction}").into_bytes();
    let keep = integer.len() as i32 + places;
    if keep < 0 {
        digits = vec![b'0'];
    } else if (keep as usize) < digits.len() {
        let up = digits[keep as usize] >= b'5';
        digits.truncate(keep as usize);
        if up {
            let mut carry = true;
            for d in digits.iter_mut().rev() {
                if *d == b'9' {
                    *d = b'0';
                } else {
                    *d += 1;
                    carry = false;
                    break;
                }
            }
            if carry {
                digits.insert(0, b'1');
            }
        }
        if digits.is_empty() {
            digits.push(b'0');
        }
    } else {
        digits.resize(keep as usize, b'0');
    }
    let nonzero = digits.iter().any(|d| *d != b'0');
    let mut result = digits.into_iter().map(char::from).collect::<String>();
    if places > 0 {
        let width = places as usize + 1;
        if result.len() < width {
            result = "0".repeat(width - result.len()) + &result;
        }
        result.insert(result.len() - places as usize, '.');
    } else if places < 0 {
        result.push_str(&"0".repeat((-places) as usize));
    }
    if value < 0.0 && nonzero {
        result.insert(0, '-');
    }
    result
}

fn exponent(value: f64) -> Option<i32> {
    if value == 0.0 {
        Some(0)
    } else {
        format!("{:e}", value.abs()).split_once('e')?.1.parse().ok()
    }
}

pub fn calculate(expression: &str) -> Option<(String, String)> {
    if expression.len() > 4096 {
        return None;
    }
    let mut parser = Parser {
        input: expression,
        pos: 0,
        depth: 0,
        significant: 0,
    };
    let value = parser.additive()?;
    parser.skip();
    if parser.pos != expression.len() || !value.is_finite() {
        return None;
    }
    let precise = if value == 0.0 {
        "0".to_owned()
    } else if value.abs() >= 1e21 || value.abs() < 1e-6 {
        let s = format!("{value:e}");
        let (m, e) = s.split_once('e')?;
        format!("{m}e{}{e}", if e.starts_with('-') { "" } else { "+" })
    } else {
        value.to_string()
    };
    let mut places = 2;
    if expression.contains('.') {
        let exp = exponent(value)?;
        places = parser.significant as i32 - exp;
        let rounded: f64 = round_decimal(value, places).parse().ok()?;
        if rounded != 0.0 && rounded.is_finite() {
            places -= (exponent(rounded)? - exp).max(0);
        }
    }
    Some((
        group(round_decimal(value, places), expression),
        group(precise, expression),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rounding() {
        for (input, expected) in [
            ("1+2/3", "1.67"),
            ("1+2", "3.00"),
            ("1/8", "0.13"),
            ("201/200", "1.01"),
            ("-201/200", "-1.01"),
            ("1/1000", "0.00"),
            ("-1/1000", "0.00"),
            ("1,000+2/3", "1,000.67"),
            ("1.0+2/3", "1.67"),
            ("1.00+2/3", "1.667"),
            ("0.00120/7", "0.0001714"),
            ("1.20+2.345", "3.5450"),
            ("9.99+0.0099", "10.00"),
            ("1.20-1.20", "0.000"),
            ("1000+0.1", "1000.1"),
            ("1,000.0/3", "333.333"),
        ] {
            assert_eq!(calculate(input).unwrap().0, expected, "{input}");
        }
        assert_eq!(calculate("1+2/3").unwrap().1, "1.6666666666666665");
    }
    #[test]
    fn arithmetic() {
        for (input, expected) in [
            ("81,629\t+512,476 +12,654,946 -3,182,432", "10,066,619"),
            ("1,000\n+250 -50", "1,200"),
            ("1000+250-50", "1200"),
            ("(1+2) x 3", "9"),
            ("2+3*4", "14"),
            ("7/2", "3.5"),
            ("2^3^2", "512"),
            ("2**5", "32"),
            ("-2^2", "-4"),
            ("2^-2", "0.25"),
            ("1,000.5*2", "2,001"),
            ("-0", "0"),
            ("-1,000+1", "-999"),
            (" \t(1+2)\n ", "3"),
        ] {
            assert_eq!(calculate(input).unwrap().1, expected, "{input}");
        }
        for input in [
            "",
            " ",
            "1,00+2",
            "1+",
            "(1+2",
            "1/0",
            "1 2+3",
            "1,\t000+1",
            "1. 5+1",
            "2 * * 3",
            "2^99999",
            "process.exit()",
        ] {
            assert!(calculate(input).is_none(), "{input}");
        }
        assert!(calculate(&"(".repeat(4096)).is_none());
        assert!(calculate(&"1".repeat(4097)).is_none());
    }
    #[test]
    fn lines_and_fences() {
        for (input, expected) in [
            ("a\nb\na", "a\nb"),
            ("a\nb\na\n", "a\nb\n"),
            ("a\r\nb\r\na\r\n", "a\r\nb\r\n"),
            ("a\r\na", "a"),
            ("a\nA\n a\na", "a\nA\n a"),
            ("\n\na\n", "\na\n"),
            ("", ""),
        ] {
            assert_eq!(unique_lines(input), expected);
        }
        let text = "before\n```js\none\ntwo\n```\nbetween\n~~~\nthree\n~~~\nafter";
        for row in [1, 2, 3] {
            assert_eq!(code_block(text, row).as_deref(), Some("one\ntwo"));
        }
        for row in [0, 4, 5, 8, 9, 20] {
            assert!(code_block(text, row).is_none());
        }
        assert_eq!(code_block(text, 7).as_deref(), Some("three"));
        assert_eq!(code_block("```\n```", 0).as_deref(), Some(""));
        assert!(code_block("```\nunfinished", 1).is_none());
        assert!(code_block("```\nx\n~~~", 1).is_none());
        assert_eq!(
            code_block("````md\n```js\nx\n```\n~~~\n````", 2).as_deref(),
            Some("```js\nx\n```\n~~~")
        );
        assert_eq!(
            code_block("  ~~~sh\n  x\n  ~~~~  ", 1).as_deref(),
            Some("  x")
        );
    }
}
