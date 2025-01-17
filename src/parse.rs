use crate::errors::{Error, Result};
use std::{collections::HashMap, mem};

// for readability's sake
pub type ParsedLine = Result<Option<(String, String)>>;

pub fn parse_line(
    line: &str,
    substitution_data: &mut HashMap<String, Option<String>>,
) -> ParsedLine {
    let mut parser = LineParser::new(line, substitution_data);
    parser.parse_line()
}

struct LineParser<'a> {
    original_line: &'a str,
    substitution_data: &'a mut HashMap<String, Option<String>>,
    line: &'a str,
    pos: usize,
}

impl<'a> LineParser<'a> {
    fn new(
        line: &'a str,
        substitution_data: &'a mut HashMap<String, Option<String>>,
    ) -> LineParser<'a> {
        LineParser {
            original_line: line,
            substitution_data,
            line: line.trim_end(), // we don’t want trailing whitespace
            pos: 0,
        }
    }

    fn err(&self) -> Error {
        Error::LineParse(self.original_line.into(), self.pos)
    }

    fn parse_line(&mut self) -> ParsedLine {
        self.skip_whitespace();
        // if its an empty line or a comment, skip it
        if self.line.is_empty() || self.line.starts_with('#') {
            return Ok(None);
        }

        let mut key = self.parse_key()?;
        self.skip_whitespace();

        // export can be either an optional prefix or a key itself
        if key == "export" {
            // here we check for an optional `=`, below we throw directly when it’s not found.
            if self.expect_equal().is_err() {
                key = self.parse_key()?;
                self.skip_whitespace();
                self.expect_equal()?;
            }
        } else {
            self.expect_equal()?;
        }
        self.skip_whitespace();

        if self.line.is_empty() || self.line.starts_with('#') {
            self.substitution_data.insert(key.clone(), None);
            return Ok(Some((key, String::new())));
        }

        let parsed_value = parse_value(self.line, self.substitution_data)?;
        self.substitution_data
            .insert(key.clone(), Some(parsed_value.clone()));

        Ok(Some((key, parsed_value)))
    }

    fn parse_key(&mut self) -> Result<String> {
        if !self
            .line
            .starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        {
            return Err(self.err());
        }
        let index = match self
            .line
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
        {
            Some(index) => index,
            None => self.line.len(),
        };
        self.pos += index;
        let key = String::from(&self.line[..index]);
        self.line = &self.line[index..];
        Ok(key)
    }

    fn expect_equal(&mut self) -> Result<()> {
        if !self.line.starts_with('=') {
            return Err(self.err());
        }
        self.line = &self.line[1..];
        self.pos += 1;
        Ok(())
    }

    fn skip_whitespace(&mut self) {
        let (pos, line) = self.line.find(|c: char| !c.is_whitespace()).map_or_else(
            || (self.line.len(), ""),
            |i| (self.pos + i, &self.line[i..]),
        );

        self.pos += pos;
        self.line = line;
    }
}

#[derive(Eq, PartialEq, Default)]
enum SubstitutionMode {
    #[default]
    None,
    Block,
    EscapedBlock,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
struct ValueState {
    strong_quote: bool,
    weak_quote: bool,
    escaped: bool,
    expecting_end: bool,
    substitution_mode: SubstitutionMode,
    substitution_name: String,
    output: String,
}

impl ValueState {
    pub fn append(&mut self, c: char) {
        self.output.push(c);
    }
}

// TODO(brxken128): clean this up 💀
#[allow(clippy::too_many_lines)]
fn parse_value(input: &str, substitution_data: &HashMap<String, Option<String>>) -> Result<String> {
    let mut state = ValueState::default();

    for (index, c) in input.chars().enumerate() {
        //the regex _should_ already trim whitespace off the end
        //expecting_end is meant to permit: k=v #comment
        //without affecting: k=v#comment
        //and throwing on: k=v w
        if state.expecting_end {
            if c == ' ' || c == '\t' {
                continue;
            } else if c == '#' {
                break;
            }

            return Err(Error::LineParse(input.to_owned(), index));
        } else if state.escaped {
            //TODO I tried handling literal \r but various issues
            //imo not worth worrying about until there's a use case
            //(actually handling backslash 0x10 would be a whole other matter)
            //then there's \v \f bell hex... etc
            match c {
                '\\' | '\'' | '"' | '$' | ' ' => state.append(c),
                'n' => state.append('\n'), // handle \n case
                _ => {
                    return Err(Error::LineParse(input.to_owned(), index));
                }
            }

            state.escaped = false;
        } else if state.strong_quote {
            if c == '\'' {
                state.strong_quote = false;
            } else {
                state.append(c);
            }
        } else if state.substitution_mode != SubstitutionMode::None {
            if c.is_alphanumeric() {
                state.substitution_name.push(c);
            } else {
                match state.substitution_mode {
                    SubstitutionMode::None => unreachable!(),
                    SubstitutionMode::Block => {
                        if c == '{' && state.substitution_name.is_empty() {
                            state.substitution_mode = SubstitutionMode::EscapedBlock;
                        } else {
                            apply_substitution(
                                substitution_data,
                                &mem::take(&mut state.substitution_name),
                                &mut state.output,
                            );
                            if c == '$' {
                                state.substitution_mode = if !state.strong_quote && !state.escaped {
                                    SubstitutionMode::Block
                                } else {
                                    SubstitutionMode::None
                                }
                            } else {
                                state.substitution_mode = SubstitutionMode::None;
                                state.append(c);
                            }
                        }
                    }
                    SubstitutionMode::EscapedBlock => {
                        if c == '}' {
                            state.substitution_mode = SubstitutionMode::None;
                            apply_substitution(
                                substitution_data,
                                &mem::take(&mut state.substitution_name),
                                &mut state.output,
                            );
                        } else {
                            state.substitution_name.push(c);
                        }
                    }
                }
            }
        } else if c == '$' {
            state.substitution_mode = if !state.strong_quote && !state.escaped {
                SubstitutionMode::Block
            } else {
                SubstitutionMode::None
            }
        } else if state.weak_quote {
            if c == '"' {
                state.weak_quote = false;
            } else if c == '\\' {
                state.escaped = true;
            } else {
                state.append(c);
            }
        } else if c == '\'' {
            state.strong_quote = true;
        } else if c == '"' {
            state.weak_quote = true;
        } else if c == '\\' {
            state.escaped = true;
        } else if c == ' ' || c == '\t' {
            state.expecting_end = true;
        } else {
            state.append(c);
        }
    }

    //XXX also fail if escaped? or...
    if state.substitution_mode == SubstitutionMode::EscapedBlock
        || state.strong_quote
        || state.weak_quote
    {
        Err(Error::LineParse(
            input.to_owned(),
            if input.is_empty() { 0 } else { input.len() - 1 },
        ))
    } else {
        apply_substitution(
            substitution_data,
            &mem::take(&mut state.substitution_name),
            &mut state.output,
        );
        Ok(state.output)
    }
}

fn apply_substitution(
    substitution_data: &HashMap<String, Option<String>>,
    substitution_name: &str,
    output: &mut String,
) {
    if let Ok(environment_value) = std::env::var(substitution_name) {
        output.push_str(&environment_value);
    } else {
        substitution_data
            .get(substitution_name)
            .map(|x| x.clone().map(|x| output.push_str(&x)));
    };
}

#[cfg(test)]
mod test {
    use crate::{errors::Error::LineParse, iter::Iter, Result};

    fn assert_parsed_string(input_string: &str, expected_parse_result: Vec<(&str, &str)>) {
        let actual_iter = Iter::new(input_string.as_bytes());
        let expected_count = &expected_parse_result.len();

        let expected_iter = expected_parse_result
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()));

        let mut count = 0;

        for (expected, actual) in expected_iter.zip(actual_iter) {
            assert!(actual.is_ok());
            assert_eq!(expected, actual.ok().unwrap());
            count += 1;
        }

        assert_eq!(count, *expected_count);
    }

    #[test]
    fn test_parse_line_env() {
        // Note 5 spaces after 'KEY8=' below
        let actual_iter = Iter::new(
            r#"
KEY=1
KEY2="2"
KEY3='3'
KEY4='fo ur'
KEY5="fi ve"
KEY6=s\ ix
KEY7=
KEY8=     
KEY9=   # foo
KEY10  ="whitespace before ="
KEY11=    "whitespace after ="
export="export as key"
export   SHELL_LOVER=1
"#
            .as_bytes(),
        );

        let expected_iter = vec![
            ("KEY", "1"),
            ("KEY2", "2"),
            ("KEY3", "3"),
            ("KEY4", "fo ur"),
            ("KEY5", "fi ve"),
            ("KEY6", "s ix"),
            ("KEY7", ""),
            ("KEY8", ""),
            ("KEY9", ""),
            ("KEY10", "whitespace before ="),
            ("KEY11", "whitespace after ="),
            ("export", "export as key"),
            ("SHELL_LOVER", "1"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()));

        let mut count = 0;
        for (expected, actual) in expected_iter.zip(actual_iter) {
            assert!(actual.is_ok());
            assert_eq!(expected, actual.ok().unwrap());
            count += 1;
        }

        assert_eq!(count, 13);
    }

    #[test]
    fn test_parse_line_comment() {
        let result: Result<Vec<(String, String)>> = Iter::new(
            br"
# foo=bar
#    "
                .as_ref(),
        )
        .collect();
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_parse_line_invalid() {
        // Note 4 spaces after 'invalid' below
        let actual_iter = Iter::new(
            r"
  invalid    
very bacon = yes indeed
=value"
                .as_bytes(),
        );

        let mut count = 0;
        for actual in actual_iter {
            assert!(actual.is_err());
            count += 1;
        }
        assert_eq!(count, 3);
    }

    #[test]
    fn test_parse_value_escapes() {
        let actual_iter = Iter::new(
            r#"
KEY=my\ cool\ value
KEY2=\$sweet
KEY3="awesome stuff \"mang\""
KEY4='sweet $\fgs'\''fds'
KEY5="'\"yay\\"\ "stuff"
KEY6="lol" #well you see when I say lol wh
KEY7="line 1\nline 2"
"#
            .as_bytes(),
        );

        let expected_iter = vec![
            ("KEY", r"my cool value"),
            ("KEY2", r"$sweet"),
            ("KEY3", r#"awesome stuff "mang""#),
            ("KEY4", r"sweet $\fgs'fds"),
            ("KEY5", r#"'"yay\ stuff"#),
            ("KEY6", "lol"),
            ("KEY7", "line 1\nline 2"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()));

        for (expected, actual) in expected_iter.zip(actual_iter) {
            assert!(actual.is_ok());
            assert_eq!(expected, actual.unwrap());
        }
    }

    #[test]
    fn test_parse_value_escapes_invalid() {
        let actual_iter = Iter::new(
            r#"
KEY=my uncool value
KEY2="why
KEY3='please stop''
KEY4=h\8u
"#
            .as_bytes(),
        );

        for actual in actual_iter {
            assert!(actual.is_err());
        }
    }

    #[test]
    fn variable_in_parenthesis_surrounded_by_quotes() {
        assert_parsed_string(
            r#"
            KEY=test
            KEY1="${KEY}"
            "#,
            vec![("KEY", "test"), ("KEY1", "test")],
        );
    }

    #[test]
    fn substitute_undefined_variables_to_empty_string() {
        assert_parsed_string(r#"KEY=">$KEY1<>${KEY2}<""#, vec![("KEY", "><><")]);
    }

    #[test]
    fn do_not_substitute_variables_with_dollar_escaped() {
        assert_parsed_string(
            "KEY=>\\$KEY1<>\\${KEY2}<",
            vec![("KEY", ">$KEY1<>${KEY2}<")],
        );
    }

    #[test]
    fn do_not_substitute_variables_in_weak_quotes_with_dollar_escaped() {
        assert_parsed_string(
            r#"KEY=">\$KEY1<>\${KEY2}<""#,
            vec![("KEY", ">$KEY1<>${KEY2}<")],
        );
    }

    #[test]
    fn do_not_substitute_variables_in_strong_quotes() {
        assert_parsed_string("KEY='>${KEY1}<>$KEY2<'", vec![("KEY", ">${KEY1}<>$KEY2<")]);
    }

    #[test]
    fn same_variable_reused() {
        assert_parsed_string(
            r"
    KEY=VALUE
    KEY1=$KEY$KEY
    ",
            vec![("KEY", "VALUE"), ("KEY1", "VALUEVALUE")],
        );
    }

    #[test]
    fn with_dot() {
        assert_parsed_string(
            r"
    KEY.Value=VALUE
    ",
            vec![("KEY.Value", "VALUE")],
        );
    }

    #[test]
    fn recursive_substitution() {
        assert_parsed_string(
            r"
            KEY=${KEY1}+KEY_VALUE
            KEY1=${KEY}+KEY1_VALUE
            ",
            vec![("KEY", "+KEY_VALUE"), ("KEY1", "+KEY_VALUE+KEY1_VALUE")],
        );
    }

    #[test]
    fn variable_without_parenthesis_is_substituted_before_separators() {
        assert_parsed_string(
            r#"
            KEY1=test_user
            KEY1_1=test_user_with_separator
            KEY=">$KEY1_1<>$KEY1}<>$KEY1{<"
            "#,
            vec![
                ("KEY1", "test_user"),
                ("KEY1_1", "test_user_with_separator"),
                ("KEY", ">test_user_1<>test_user}<>test_user{<"),
            ],
        );
    }

    #[test]
    fn substitute_variable_from_env_variable() {
        std::env::set_var("KEY11", "test_user_env");

        assert_parsed_string(r#"KEY=">${KEY11}<""#, vec![("KEY", ">test_user_env<")]);
    }

    #[test]
    fn substitute_variable_env_variable_overrides_dotenv_in_substitution() {
        std::env::set_var("KEY11", "test_user_env");

        assert_parsed_string(
            r#"
    KEY11=test_user
    KEY=">${KEY11}<"
    "#,
            vec![("KEY11", "test_user"), ("KEY", ">test_user_env<")],
        );
    }

    #[test]
    fn consequent_substitutions() {
        assert_parsed_string(
            r"
    KEY1=test_user
    KEY2=$KEY1_2
    KEY=>${KEY1}<>${KEY2}<
    ",
            vec![
                ("KEY1", "test_user"),
                ("KEY2", "test_user_2"),
                ("KEY", ">test_user<>test_user_2<"),
            ],
        );
    }

    #[test]
    fn consequent_substitutions_with_one_missing() {
        assert_parsed_string(
            r"
    KEY2=$KEY1_2
    KEY=>${KEY1}<>${KEY2}<
    ",
            vec![("KEY2", "_2"), ("KEY", "><>_2<")],
        );
    }

    #[test]
    fn should_not_parse_unfinished_substitutions() {
        let wrong_value = ">${KEY{<";

        let parsed_values: Vec<_> = Iter::new(
            format!(
                r"
    KEY=VALUE
    KEY1={wrong_value}
    "
            )
            .as_bytes(),
        )
        .collect();

        assert_eq!(parsed_values.len(), 2);

        parsed_values[0].as_ref().map_or_else(
            |_| panic!("Expected the first value to be parsed"),
            |first_line| assert_eq!(first_line, &(String::from("KEY"), String::from("VALUE"))),
        );

        if let Err(LineParse(second_value, index)) = &parsed_values[1] {
            assert_eq!(second_value, wrong_value);
            assert_eq!(*index, wrong_value.len() - 1);
        }
    }

    #[test]
    fn should_not_allow_dot_as_first_character_of_key() {
        let wrong_key_value = ".Key=VALUE";

        let parsed_values: Vec<_> = Iter::new(wrong_key_value.as_bytes()).collect();

        assert_eq!(parsed_values.len(), 1);

        if let Err(LineParse(second_value, index)) = &parsed_values[0] {
            assert_eq!(second_value, wrong_key_value);
            assert_eq!(*index, 0);
        }
    }

    #[test]
    fn should_not_parse_illegal_format() {
        let wrong_format = r"<><><>";
        let parsed_values: Vec<_> = Iter::new(wrong_format.as_bytes()).collect();

        assert_eq!(parsed_values.len(), 1);

        if let Err(LineParse(wrong_value, index)) = &parsed_values[0] {
            assert_eq!(wrong_value, wrong_format);
            assert_eq!(*index, 0);
        }
    }

    #[test]
    fn should_not_parse_illegal_escape() {
        let wrong_escape = r">\f<";
        let parsed_values: Vec<_> = Iter::new(format!("VALUE={wrong_escape}").as_bytes()).collect();

        assert_eq!(parsed_values.len(), 1);

        if let Err(LineParse(wrong_value, index)) = &parsed_values[0] {
            assert_eq!(wrong_value, wrong_escape);
            assert_eq!(*index, wrong_escape.find('\\').unwrap() + 1);
        }
    }
}
