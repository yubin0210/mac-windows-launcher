//! Spotlight pure algorithmic search and math evaluation core.
//!
//! Provides:
//! - High-performance fuzzy matching with prefix, word boundary, acronym, and case match bonuses.
//! - Safe arithmetic expression evaluator (+, -, *, /, ^, parentheses, sqrt, abs, trig, log).
//! - Domain models for Spotlight search items and categories.

use serde::{Deserialize, Serialize};

/// Categories of Spotlight search items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpotlightCategory {
    Application,
    SystemSetting,
    Calculation,
    File,
    Other,
}

/// An item indexed or presented by Spotlight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpotlightItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub category: SpotlightCategory,
    pub action_target: String,
}

impl SpotlightItem {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        subtitle: impl Into<String>,
        category: SpotlightCategory,
        action_target: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: subtitle.into(),
            category,
            action_target: action_target.into(),
        }
    }

    /// Match a query against this item's title using fuzzy matching.
    pub fn match_query(&self, query: &str) -> Option<i32> {
        fuzzy_match(query, &self.title)
    }
}

/// Computes a fuzzy matching score between `query` and `target`.
///
/// Returns `Some(score)` if all characters in `query` appear in `target`
/// in sequence (case-insensitive).
/// Returns `None` if characters cannot be matched in sequence.
///
/// Scoring bonuses:
/// - Prefix match bonus (+150 if matching at beginning of target)
/// - Word boundary bonus (+60 if matching after space, hyphen, underscore, slash, or CamelCase)
/// - Acronym match bonus (+80 if consecutive query characters match word boundaries)
/// - Consecutive match bonus (+40 if query characters match adjacent target characters)
/// - Exact case match bonus (+5 if character cases match)
/// - Distance/gap penalties for scattered matches
pub fn fuzzy_match(query: &str, target: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    if target.is_empty() {
        return None;
    }

    let q_chars: Vec<char> = query.chars().collect();
    let t_chars: Vec<char> = target.chars().collect();

    let q_lower: Vec<char> = q_chars.iter().map(|c| c.to_ascii_lowercase()).collect();
    let t_lower: Vec<char> = t_chars.iter().map(|c| c.to_ascii_lowercase()).collect();

    let m = q_lower.len();
    let n = t_lower.len();

    if m > n {
        return None;
    }

    // Fast subsequence rejection
    let mut q_idx = 0;
    for &tc in &t_lower {
        if q_idx < m && tc == q_lower[q_idx] {
            q_idx += 1;
        }
    }
    if q_idx < m {
        return None;
    }

    // Precompute word boundaries on original target
    let mut is_boundary = vec![false; n];
    for j in 0..n {
        is_boundary[j] = is_word_boundary(&t_chars, j);
    }

    // DP table: dp[i][j] = maximum score matching q[0..=i] ending with target[j].
    let mut dp: Vec<Vec<Option<i32>>> = vec![vec![None; n]; m];

    // Base case: i = 0
    for j in 0..n {
        if q_lower[0] == t_lower[j] {
            let mut score = 10;
            if j == 0 {
                score += 150; // Prefix bonus
                score += 60;  // Start of string boundary
            } else if is_boundary[j] {
                score += 60; // Word boundary bonus
            }

            // Leading gap penalty
            let lead_penalty = (j as i32 * 3).min(50);
            score -= lead_penalty;

            if q_chars[0] == t_chars[j] {
                score += 5; // Exact case match
            }
            dp[0][j] = Some(score);
        }
    }

    // Inductive step: i > 0
    for i in 1..m {
        for j in i..n {
            if q_lower[i] == t_lower[j] {
                let mut best_prev: Option<i32> = None;
                for k in (i - 1)..j {
                    if let Some(prev_score) = dp[i - 1][k] {
                        let mut step = 10; // Base match
                        if is_boundary[j] {
                            step += 60;
                            if is_boundary[k] {
                                step += 80; // Acronym match across word boundaries
                            }
                        }

                        if k + 1 == j {
                            step += 40; // Consecutive match bonus
                        } else {
                            let gap = (j - 1 - k) as i32;
                            step -= (gap * 2).min(30); // Gap penalty
                        }

                        if q_chars[i] == t_chars[j] {
                            step += 5; // Exact case match
                        }

                        let total = prev_score + step;
                        best_prev = match best_prev {
                            Some(cur) => Some(cur.max(total)),
                            None => Some(total),
                        };
                    }
                }
                dp[i][j] = best_prev;
            }
        }
    }

    // Find the best final match across all ending positions
    let mut max_score: Option<i32> = None;
    for j in (m - 1)..n {
        if let Some(score) = dp[m - 1][j] {
            let trailing_gap = ((n - 1 - j) as i32).min(30);
            let mut final_score = score - trailing_gap;
            if m == n {
                final_score += 100; // Exact full match bonus
            }
            max_score = match max_score {
                Some(cur) => Some(cur.max(final_score)),
                None => Some(final_score),
            };
        }
    }

    max_score
}

fn is_word_boundary(chars: &[char], j: usize) -> bool {
    if j == 0 {
        return true;
    }
    let prev = chars[j - 1];
    let curr = chars[j];

    // Delimiter boundary
    if (prev == ' ' || prev == '-' || prev == '_' || prev == '.' || prev == '/' || prev == '\\' || prev == ':')
        && curr.is_alphanumeric()
    {
        return true;
    }

    // CamelCase: lower followed by upper
    if prev.is_ascii_lowercase() && curr.is_ascii_uppercase() {
        return true;
    }

    // Boundary between digits and alphabetic characters
    if (prev.is_alphabetic() && curr.is_ascii_digit())
        || (prev.is_ascii_digit() && curr.is_alphabetic())
    {
        return true;
    }

    false
}

/// Evaluates a mathematical expression string.
///
/// Supports:
/// - Basic arithmetic: `+`, `-`, `*`, `/`
/// - Exponentiation: `^`
/// - Grouping: parentheses `(...)`
/// - Functions: `sqrt(...)`, `abs(...)`, `sin(...)`, `cos(...)`, `tan(...)`, `ln(...)`, `log(...)`, `floor(...)`, `ceil(...)`, `round(...)`
/// - Constants: `pi`, `e`
///
/// Returns `None` if the input is empty, not a valid expression, divides by zero,
/// exceeds 128 recursive grouping/sign/power levels, or produces a non-finite float.
pub fn evaluate_math(expr: &str) -> Option<f64> {
    let tokens = tokenize(expr)?;
    let mut parser = MathParser::new(tokens);
    let result = parser.parse_expression()?;
    if parser.pos != parser.tokens.len() {
        return None; // Unparsed trailing tokens
    }
    if !result.is_finite() {
        return None;
    }
    Some(result)
}

#[derive(Debug, PartialEq, Clone)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        match c {
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '^' => {
                tokens.push(Token::Caret);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '0'..='9' | '.' => {
                let start = i;
                let mut has_dot = c == '.';
                i += 1;
                while i < len {
                    if chars[i].is_ascii_digit() {
                        i += 1;
                    } else if chars[i] == '.' {
                        if has_dot {
                            return None; // Double dot syntax error
                        }
                        has_dot = true;
                        i += 1;
                    } else {
                        break;
                    }
                }
                let s: String = chars[start..i].iter().collect();
                if s == "." {
                    return None;
                }
                let val = s.parse::<f64>().ok()?;
                if !val.is_finite() {
                    return None;
                }
                tokens.push(Token::Number(val));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let start = i;
                i += 1;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                tokens.push(Token::Ident(s.to_ascii_lowercase()));
            }
            _ => return None, // Unknown symbol
        }
    }

    if tokens.is_empty() {
        return None;
    }

    Some(tokens)
}

struct MathParser {
    tokens: Vec<Token>,
    pos: usize,
    depth: usize,
}

impl MathParser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0, depth: 0 }
    }

    // Bound recursive grouping, unary signs and right-associative powers while
    // allowing long flat expressions to use their existing iterative loops.
    fn nested(&mut self, parse: impl FnOnce(&mut Self) -> Option<f64>) -> Option<f64> {
        if self.depth >= 128 { return None; }
        self.depth += 1;
        let value = parse(self);
        self.depth -= 1;
        value
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn parse_expression(&mut self) -> Option<f64> {
        let mut left = self.parse_term()?;

        while let Some(tok) = self.peek() {
            match tok {
                Token::Plus => {
                    self.next();
                    let right = self.parse_term()?;
                    left += right;
                    if !left.is_finite() {
                        return None;
                    }
                }
                Token::Minus => {
                    self.next();
                    let right = self.parse_term()?;
                    left -= right;
                    if !left.is_finite() {
                        return None;
                    }
                }
                _ => break,
            }
        }

        Some(left)
    }

    fn parse_term(&mut self) -> Option<f64> {
        let mut left = self.parse_power()?;

        while let Some(tok) = self.peek() {
            match tok {
                Token::Star => {
                    self.next();
                    let right = self.parse_power()?;
                    left *= right;
                    if !left.is_finite() {
                        return None;
                    }
                }
                Token::Slash => {
                    self.next();
                    let right = self.parse_power()?;
                    if right == 0.0 {
                        return None; // Guard against division by zero
                    }
                    left /= right;
                    if !left.is_finite() {
                        return None;
                    }
                }
                _ => break,
            }
        }

        Some(left)
    }

    fn parse_power(&mut self) -> Option<f64> {
        let base = self.parse_unary()?;

        if let Some(Token::Caret) = self.peek() {
            self.next();
            // Right-associative exponentiation: base ^ power
            let exp = self.nested(Self::parse_power)?;
            let result = base.powf(exp);
            if !result.is_finite() {
                return None;
            }
            Some(result)
        } else {
            Some(base)
        }
    }

    fn parse_unary(&mut self) -> Option<f64> {
        match self.peek() {
            Some(Token::Plus) => {
                self.next();
                self.nested(Self::parse_unary)
            }
            Some(Token::Minus) => {
                self.next();
                let val = self.nested(Self::parse_unary)?;
                let res = -val;
                if !res.is_finite() {
                    None
                } else {
                    Some(res)
                }
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_single_arg_func<F>(&mut self, f: F) -> Option<f64>
    where
        F: FnOnce(f64) -> Option<f64>,
    {
        match self.next()? {
            Token::LParen => {
                let arg = self.nested(Self::parse_expression)?;
                match self.next()? {
                    Token::RParen => {
                        let res = f(arg)?;
                        if res.is_finite() {
                            Some(res)
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn parse_primary(&mut self) -> Option<f64> {
        match self.next()? {
            Token::Number(val) => Some(val),
            Token::LParen => {
                let val = self.nested(Self::parse_expression)?;
                match self.next()? {
                    Token::RParen => Some(val),
                    _ => None,
                }
            }
            Token::Ident(name) => match name.as_str() {
                "pi" => Some(std::f64::consts::PI),
                "e" => Some(std::f64::consts::E),
                "sqrt" => self.parse_single_arg_func(|x| if x >= 0.0 { Some(x.sqrt()) } else { None }),
                "abs" => self.parse_single_arg_func(|x| Some(x.abs())),
                "sin" => self.parse_single_arg_func(|x| Some(x.sin())),
                "cos" => self.parse_single_arg_func(|x| Some(x.cos())),
                "tan" => self.parse_single_arg_func(|x| Some(x.tan())),
                "ln" => self.parse_single_arg_func(|x| if x > 0.0 { Some(x.ln()) } else { None }),
                "log" | "log10" => self.parse_single_arg_func(|x| if x > 0.0 { Some(x.log10()) } else { None }),
                "floor" => self.parse_single_arg_func(|x| Some(x.floor())),
                "ceil" => self.parse_single_arg_func(|x| Some(x.ceil())),
                "round" => self.parse_single_arg_func(|x| Some(x.round())),
                _ => None,
            },
            _ => None,
        }
    }
}
