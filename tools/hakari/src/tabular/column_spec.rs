use self::{Alignment::*, ColumnSpec::*};
use super::error::*;

#[derive(Clone)]
pub enum ColumnSpec {
    Align(Alignment),
    Literal(String),
}

#[derive(Clone, Copy)]
pub enum Alignment {
    Left,
    Center,
    Right,
}

pub fn row_spec_to_string(specs: &[ColumnSpec]) -> String {
    let mut result = String::new();

    for spec in specs {
        match *spec {
            Align(Left) => result.push_str("{:<}"),
            Align(Center) => result.push_str("{:^}"),
            Align(Right) => result.push_str("{:>}"),
            Literal(ref literal) => {
                for c in literal.chars() {
                    match c {
                        '{' => result.push_str("{{"),
                        '}' => result.push_str("}}"),
                        _ => result.push(c),
                    }
                }
            }
        }
    }

    result
}

pub fn parse_row_spec(spec: &str) -> Result<(Vec<ColumnSpec>, usize)> {
    let mut vec = Vec::new();
    let mut count = 0;
    let mut buf = String::new();

    let mut chars = spec.chars();

    while let Some(c) = chars.next() {
        let mut align = |buf: &mut String, col_spec: ColumnSpec| {
            if !buf.is_empty() {
                vec.push(Literal(std::mem::take(buf)));
            }
            vec.push(col_spec);
            count += 1;
        };

        // Should be generating this.
        match c {
            '{' => match chars.next() {
                None => return Err(Error::UnclosedColumnSpec(String::new())),
                Some('{') => buf.push('{'),
                Some(':') => match chars.next() {
                    None => return Err(Error::UnclosedColumnSpec(":".to_owned())),
                    Some(c) => {
                        let alignment = match c {
                            '<' => Left,
                            '>' => Right,
                            '^' => Center,
                            _ => return Err(Error::UnexpectedCharacter(c)),
                        };

                        match chars.next() {
                            Some('}') => align(&mut buf, Align(alignment)),
                            Some(c) => return Err(Error::UnexpectedCharacter(c)),
                            None => return Err(Error::UnclosedColumnSpec(format!(":{}", c))),
                        }
                    }
                },
                Some(c) => return Err(Error::UnexpectedCharacter(c)),
            },

            '}' => match chars.next() {
                Some('}') => buf.push('}'),
                _ => return Err(Error::UnexpectedRightBrace),
            },

            _ => buf.push(c),
        }
    }

    if !buf.is_empty() {
        vec.push(Literal(buf));
    }

    Ok((vec, count))
}
