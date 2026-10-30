// SPDX-FileCopyrightText: Copyright (c) 2026 Wataru Ashihara <wataash0607@gmail.com>
// SPDX-License-Identifier: Apache-2.0
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn code_fences() {
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
