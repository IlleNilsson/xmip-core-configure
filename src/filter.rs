//! A Subscription's filter as a designer edits it (ADR-0064): rows and
//! groups, a view of the compiled expression.
//!
//! The filter is one line of Xmip's expression language
//! (`path::expression`, ADR-0066), compiled when the Application is
//! read; nothing here parses or prints it. What is here is the classic filter
//! dialog over it: **rows** of property, operator and value, gathered in And
//! and Or groups, with Not around any of them. [`structure`] and [`rows`] give
//! a filter's rows; [`expression`] and [`text`] turn rows back into the tree
//! and its canonical line. A designer edits rows of words without writing
//! the language, and the text is rewritten — into the one canonical form —
//! only when the designer changes it; reading never reformats a line a
//! developer wrote.
//!
//! Every condition the language has is a row (ADR-0066 clause 3): its left
//! operand is the row's property, written in the language; its operator is
//! one of the language's [`path::expression::OPERATORS`]; its value is a literal
//! of one of [`kinds`], or — kind `expression` — anything the language can
//! write there, such as another name or the list an `in` holds.

use path::expression::{Comparison, Condition, Expression, KINDS, Operand, Value};
use serde::{Deserialize, Serialize};

/// The row kind whose value is written in the expression language itself:
/// a name, a computed value, or the list an `in` holds.
pub const EXPRESSION: &str = "expression";

/// One part of a filter as a designer edits it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "kebab-case")]
pub enum FilterPart {
    /// A row: what is compared (`property`, in the language — usually a
    /// name such as `header:http.x-channel`), one of the language's
    /// operators, and the value, written as text and read as `kind` — one
    /// of [`kinds`]. `exists` takes no value; its `value` and `kind` are
    /// empty.
    Condition {
        property: String,
        operator: String,
        value: String,
        kind: String,
    },
    /// Parts that must all hold (`and`) or of which one must (`or`).
    Group { join: Join, parts: Vec<FilterPart> },
    /// A part that must not hold.
    Not { part: Box<FilterPart> },
}

/// How a group's parts combine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Join {
    And,
    Or,
}

/// The kinds a row's value is read as, in the order a designer offers them:
/// the language's literal kinds, then [`EXPRESSION`].
#[must_use]
pub fn kinds() -> Vec<&'static str> {
    KINDS.iter().copied().chain([EXPRESSION]).collect()
}

/// A filter's text as rows and groups.
///
/// # Errors
/// The language's sentence when the text does not compile.
pub fn structure(filter: &str) -> Result<FilterPart, String> {
    Expression::parse(filter)
        .map(|expression| rows(&expression))
        .map_err(|refused| refused.message)
}

/// A compiled filter as rows and groups. The outermost part is always a
/// group, so a designer has somewhere to add a row.
#[must_use]
pub fn rows(filter: &Expression) -> FilterPart {
    match part(filter.condition()) {
        group @ FilterPart::Group { .. } => group,
        other => FilterPart::Group {
            join: Join::And,
            parts: vec![other],
        },
    }
}

/// Rows and groups as the filter's canonical text.
///
/// # Errors
/// As [`expression`].
pub fn text(part: &FilterPart) -> Result<String, String> {
    expression(part).map(|expression| expression.text().to_string())
}

/// Rows and groups compiled, with the canonical text. A group of one part
/// is that part.
///
/// # Errors
/// A row without a property, an operator the language does not have, a
/// value that does not read as its kind, or parts whose kinds cannot meet —
/// each said in a sentence that names the row.
pub fn expression(part: &FilterPart) -> Result<Expression, String> {
    Expression::from_condition(condition(part)?).map_err(|refused| refused.message)
}

fn part(condition: &Condition) -> FilterPart {
    let row = |left: &Operand, value: String, kind: &str| FilterPart::Condition {
        property: left.to_string(),
        operator: condition.operator().unwrap_or_default().to_string(),
        value,
        kind: kind.to_string(),
    };
    let valued = |left: &Operand, right: &Operand| match right {
        Operand::Literal(literal) => row(left, literal.raw(), literal.kind()),
        other => row(left, other.to_string(), EXPRESSION),
    };

    match condition {
        Condition::All(parts) => group(Join::And, parts),
        Condition::Any(parts) => group(Join::Or, parts),
        Condition::Not(inner) => FilterPart::Not {
            part: Box::new(part(inner)),
        },
        Condition::Compare { left, right, .. } => valued(left, right),
        Condition::Like { value, pattern, .. } => valued(value, pattern),
        Condition::In { value, list, .. } => {
            let items: Vec<String> = list.iter().map(ToString::to_string).collect();
            row(value, items.join(", "), EXPRESSION)
        }
        Condition::Exists(value) => row(value, String::new(), ""),
    }
}

fn group(join: Join, parts: &[Condition]) -> FilterPart {
    FilterPart::Group {
        join,
        parts: parts.iter().map(part).collect(),
    }
}

fn condition(part: &FilterPart) -> Result<Condition, String> {
    match part {
        FilterPart::Condition {
            property,
            operator,
            value,
            kind,
        } => row(property, operator, value, kind),
        FilterPart::Group { join, parts } => {
            let mut parts = parts.iter().map(condition).collect::<Result<Vec<_>, _>>()?;
            if parts.len() == 1 {
                return Ok(parts.remove(0));
            }
            Ok(match join {
                Join::And => Condition::All(parts),
                Join::Or => Condition::Any(parts),
            })
        }
        FilterPart::Not { part } => Ok(Condition::Not(Box::new(condition(part)?))),
    }
}

fn row(property: &str, operator: &str, value: &str, kind: &str) -> Result<Condition, String> {
    if property.trim().is_empty() {
        return Err(format!(
            "a condition '{operator} {value}' requires a property"
        ));
    }
    let left = language(property, path::expression::operand)?;
    let right = || compared(property, value, kind);
    let negated = operator.starts_with("not ");

    if let Some(comparison) = Comparison::ALL
        .into_iter()
        .find(|comparison| comparison.spelling() == operator)
    {
        return Ok(Condition::Compare {
            left,
            comparison,
            right: right()?,
        });
    }
    Ok(match operator {
        "like" | "not like" => Condition::Like {
            value: left,
            pattern: right()?,
            negated,
        },
        "in" | "not in" => Condition::In {
            value: left,
            list: if kind == EXPRESSION {
                language(value, path::expression::operands)?
            } else {
                vec![right()?]
            },
            negated,
        },
        "exists" => Condition::Exists(left),
        other => {
            return Err(format!(
                "'{other}' is not an operator; a condition takes one of {}",
                path::expression::OPERATORS.join(", ")
            ));
        }
    })
}

/// A row's value: a literal read as its kind, or the language's own text.
fn compared(property: &str, value: &str, kind: &str) -> Result<Operand, String> {
    if kind == EXPRESSION {
        language(value, path::expression::operand)
    } else {
        Value::read(property, value, kind).map(Operand::Literal)
    }
}

/// `text` read by the language, a refusal naming what was read.
fn language<T, E: std::fmt::Display>(
    text: &str,
    read: impl Fn(&str) -> Result<T, E>,
) -> Result<T, String> {
    read(text).map_err(|refused| format!("'{text}': {refused}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Filters in the canonical form: each reads to rows that write it back
    /// byte for byte.
    const CANONICAL: [&str; 14] = [
        "MessageType = 'Order' and not Amount > 1000 and header:http.x-channel = 'web'",
        "MessageType = 'Order'",
        "true",
        "false",
        "MessageType = 'Order' and (Amount > 1000 or exists header:http.x-urgent) and not \
         Customer like 'EU%'",
        "Urgent <> true and Amount <= -5",
        "Region in ('EU', 'US') or (Amount + Tax >= 2000 and Id || '-' = coalesce(Ref, 'x-'))",
        "a = 1 and (b = 2 and c = 3)",
        "not (a = 1 or b = 2)",
        "not not a = 1",
        r#""regex:OrderNo:^INV-(\d+)$" = 12345"#,
        "Name = 'O''Brien' and Amount not in (1, 2) and A = B",
        "not exists Note or Customer not like '%-00_2'",
        "(a = 1 or b = 2) and (c = 3 or d = 4)",
    ];

    #[test]
    fn canonical_text_to_rows_and_back_is_byte_exact() {
        for original in CANONICAL {
            let rows = structure(original).expect("reads");
            assert!(matches!(rows, FilterPart::Group { .. }), "{original}");
            assert_eq!(text(&rows).expect("writes"), original);
        }
    }

    #[test]
    fn any_accepted_text_to_rows_and_back_is_its_canonical_equivalent() {
        for (written, canonical) in [
            (
                "MessageType='Order'  AND Amount>1000",
                "MessageType = 'Order' and Amount > 1000",
            ),
            ("A != 1 OR B IS NULL", "A <> 1 or not exists B"),
            ("A is not null", "exists A"),
            ("a = 1 or b = 2 and c = 3", "a = 1 or (b = 2 and c = 3)"),
            ("(a = 1)", "a = 1"),
            ("\"Amount\" > - 5", "Amount > -5"),
            ("TRUE", "true"),
        ] {
            let rows = structure(written).expect("reads");
            let back = text(&rows).expect("writes");
            assert_eq!(back, canonical, "{written}");
            assert_eq!(
                Expression::parse(&back).expect("compiles"),
                Expression::parse(written).expect("compiles"),
                "{written} means what its canonical text means"
            );
        }
    }

    #[test]
    fn the_rows_are_the_filter_dialog() {
        let FilterPart::Group { join, parts } = structure(CANONICAL[4]).expect("reads") else {
            panic!("a group");
        };

        assert_eq!(join, Join::And);
        assert_eq!(
            parts[0],
            FilterPart::Condition {
                property: "MessageType".to_string(),
                operator: "=".to_string(),
                value: "Order".to_string(),
                kind: "text".to_string(),
            }
        );
        let FilterPart::Group { join, parts: inner } = &parts[1] else {
            panic!("an or group");
        };
        assert_eq!(*join, Join::Or);
        assert_eq!(
            inner[1],
            FilterPart::Condition {
                property: "header:http.x-urgent".to_string(),
                operator: "exists".to_string(),
                value: String::new(),
                kind: String::new(),
            }
        );
        assert!(matches!(&parts[2], FilterPart::Not { .. }));

        let listed = structure("Region in ('EU', 'US')").expect("reads");
        let FilterPart::Group { parts, .. } = listed else {
            panic!("a group");
        };
        assert_eq!(
            parts[0],
            FilterPart::Condition {
                property: "Region".to_string(),
                operator: "in".to_string(),
                value: "'EU', 'US'".to_string(),
                kind: EXPRESSION.to_string(),
            }
        );
        assert_eq!(kinds(), ["text", "integer", "boolean", "expression"]);
    }

    #[test]
    fn a_row_that_does_not_read_is_refused_in_words() {
        let row = |operator: &str, value: &str, kind: &str| FilterPart::Condition {
            property: "Amount".to_string(),
            operator: operator.to_string(),
            value: value.to_string(),
            kind: kind.to_string(),
        };

        assert_eq!(
            text(&row(">", "about ten", "integer")).expect_err("not a number"),
            "Amount compares with 'about ten', which is not an integer"
        );
        assert!(text(&row("==", "1", "integer")).is_err());
        assert!(text(&row("=", "1", "decimal")).is_err());
        assert!(
            text(&row("like", "1", "integer")).is_err(),
            "like reads text"
        );
        assert!(text(&row("=", "Limit +", EXPRESSION)).is_err());
        assert_eq!(
            text(&row(">", "10", "integer")).expect("reads"),
            "Amount > 10"
        );
        assert_eq!(
            text(&row("in", "EU", "text")).expect("a list of one"),
            "Amount in ('EU')"
        );
        let nameless = FilterPart::Condition {
            property: " ".to_string(),
            operator: "=".to_string(),
            value: "1".to_string(),
            kind: "integer".to_string(),
        };
        assert!(
            text(&nameless)
                .expect_err("no property")
                .contains("requires a property")
        );
    }

    #[test]
    fn text_that_does_not_compile_is_refused_in_the_language_s_words() {
        assert!(structure("{ equals = { property = \"A\" } }").is_err());
        assert!(
            structure("A == 1")
                .expect_err("==")
                .contains("equality is '='")
        );
        assert!(structure("1 = 'x'").is_err());
    }
}
