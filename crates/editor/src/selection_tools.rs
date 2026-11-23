// SPDX-FileCopyrightText: Copyright (c) 2026 Wataru Ashihara <wataash0607@gmail.com>
// SPDX-License-Identifier: Apache-2.0
pub fn code_block(text: &str, row: usize) -> Option<String> {
    let mut opening = None;
    let mut offset = 0;
    for (i, raw_line) in text.split_inclusive('\n').enumerate() {
        let line_start = offset;
        offset += raw_line.len();
        let line = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let line = line.trim_start_matches([' ', '\t']);
        let ch = line.chars().next().unwrap_or(' ');
        let count = line.chars().take_while(|c| *c == ch).count();
        if ch != '`' && ch != '~' || count < 3 {
            continue;
        }
        let rest = &line[count..];
        if let Some((start, body_start, fence, length)) = opening {
            if ch == fence && count >= length && rest.trim_matches([' ', '\t']).is_empty() {
                if row >= start && row < i {
                    let body = &text[body_start..line_start];
                    let body = body
                        .strip_suffix("\r\n")
                        .or_else(|| body.strip_suffix('\n'))
                        .unwrap_or(body);
                    return Some(body.replace("\r\n", "\n"));
                }
                if i >= row {
                    return None;
                }
                opening = None;
            }
        } else if ch != '`' || !rest.contains('`') {
            if i > row {
                return None;
            }
            opening = Some((i, offset, ch, count));
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
        assert_eq!(
            code_block("~~~\r\n日本語\r\n\r\n~~~\r\n", 2).as_deref(),
            Some("日本語\n")
        );
        let text_with_spaces = "~~~\n　~~~\n~~~　\nend\n~~~";
        assert_eq!(
            code_block(text_with_spaces, 3).as_deref(),
            Some("　~~~\n~~~　\nend")
        );
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
