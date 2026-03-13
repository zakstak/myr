use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verb {
    Focus,
    Move,
    Resize,
    Close,
    Fullscreen,
    Swap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Selector {
    Title(String),
    Class(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub verb: Verb,
    pub selector: Selector,
    pub args: Vec<String>,
}

impl Command {
    pub fn to_hyprctl(&self) -> String {
        self.to_hyprctl_with_selector(&self.selector)
    }

    pub fn to_hyprctl_fallback(&self) -> Option<String> {
        let alt = match &self.selector {
            Selector::Title(v) => Selector::Class(v.clone()),
            Selector::Class(v) => Selector::Title(v.clone()),
        };
        Some(self.to_hyprctl_with_selector(&alt))
    }

    fn to_hyprctl_with_selector(&self, selector: &Selector) -> String {
        let selector_str = match selector {
            Selector::Title(t) => format!("title:{}", t),
            Selector::Class(c) => format!("class:{}", c),
        };

        match self.verb {
            Verb::Focus => format!("dispatch focuswindow {}", selector_str),
            Verb::Move => {
                let direction = self.args.first().map(|s| s.as_str()).unwrap_or("");
                let dir_short = match direction.to_uppercase().as_str() {
                    "LEFT" => "l",
                    "RIGHT" => "r",
                    "UP" => "u",
                    "DOWN" => "d",
                    _ => "l",
                };
                format!(
                    "dispatch focuswindow {} ; dispatch movewindow {}",
                    selector_str, dir_short
                )
            }
            Verb::Resize => {
                let width_pct = self.args.first().map(|s| s.as_str()).unwrap_or("50");
                let height_pct = self.args.get(1).map(|s| s.as_str()).unwrap_or("50");
                format!(
                    "dispatch focuswindow {} ; dispatch resizeactive {}% {}%",
                    selector_str, width_pct, height_pct
                )
            }
            Verb::Close => format!("dispatch closewindow {}", selector_str),
            Verb::Fullscreen => format!(
                "dispatch focuswindow {} ; dispatch fullscreen 1",
                selector_str
            ),
            Verb::Swap => {
                let sel2_raw = self.args.first().map(|s| s.as_str()).unwrap_or("");
                format!(
                    "dispatch focuswindow {} ; dispatch swapwindow {}",
                    selector_str, sel2_raw
                )
            }
        }
    }
}

pub fn parse(input: &str) -> anyhow::Result<Vec<Command>> {
    let mut commands = Vec::new();

    for line in input.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.eq_ignore_ascii_case("NONE") {
            return Ok(vec![]);
        }

        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }

        let verb_str = tokens[0];
        let verb = match verb_str.to_uppercase().as_str() {
            "FOCUS" => Verb::Focus,
            "MOVE" => Verb::Move,
            "RESIZE" => Verb::Resize,
            "CLOSE" => Verb::Close,
            "FULLSCREEN" => Verb::Fullscreen,
            "SWAP" => Verb::Swap,
            _ => anyhow::bail!("Unknown verb: {}", verb_str),
        };

        if tokens.len() < 2 {
            anyhow::bail!("Missing selector for verb {}", verb_str);
        }

        let selector_str = tokens[1];
        let selector = parse_selector(selector_str)?;

        let args: Vec<String> = tokens[2..].iter().map(|s| s.to_string()).collect();

        match verb {
            Verb::Move => {
                if args.is_empty() {
                    anyhow::bail!("MOVE requires a direction argument");
                }
                let dir = args[0].to_uppercase();
                if !["LEFT", "RIGHT", "UP", "DOWN"].contains(&dir.as_str()) {
                    anyhow::bail!("MOVE direction must be LEFT, RIGHT, UP, or DOWN");
                }
            }
            Verb::Resize => {
                if args.len() < 2 {
                    anyhow::bail!("RESIZE requires two numeric arguments (width%, height%)");
                }
                args[0]
                    .parse::<i32>()
                    .map_err(|_| anyhow::anyhow!("RESIZE width must be numeric"))?;
                args[1]
                    .parse::<i32>()
                    .map_err(|_| anyhow::anyhow!("RESIZE height must be numeric"))?;
            }
            Verb::Swap => {
                if args.is_empty() {
                    anyhow::bail!("SWAP requires a second selector argument");
                }
                parse_selector(args[0].as_str())?;
            }
            _ => {}
        }

        commands.push(Command {
            verb,
            selector,
            args,
        });
    }

    Ok(commands)
}

fn parse_selector(s: &str) -> anyhow::Result<Selector> {
    if let Some(colon_pos) = s.find(':') {
        let (typ, value) = s.split_at(colon_pos);
        let value = &value[1..];

        match typ.to_lowercase().as_str() {
            "title" => Ok(Selector::Title(value.to_string())),
            "class" => Ok(Selector::Class(value.to_string())),
            _ => anyhow::bail!("Invalid selector type: {}", typ),
        }
    } else {
        anyhow::bail!("Selector must be in format 'type:value'")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_focus() {
        let input = "FOCUS title:Firefox";
        let result = parse(input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].verb, Verb::Focus);
        assert_eq!(result[0].selector, Selector::Title("Firefox".to_string()));
        assert!(result[0].args.is_empty());
    }

    #[test]
    fn test_parse_close() {
        let input = "CLOSE class:Alacritty";
        let result = parse(input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].verb, Verb::Close);
        assert_eq!(result[0].selector, Selector::Class("Alacritty".to_string()));
    }

    #[test]
    fn test_parse_move() {
        let input = "MOVE title:Terminal LEFT";
        let result = parse(input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].verb, Verb::Move);
        assert_eq!(result[0].selector, Selector::Title("Terminal".to_string()));
        assert_eq!(result[0].args, vec!["LEFT"]);
    }

    #[test]
    fn test_parse_resize() {
        let input = "RESIZE class:Chrome 1920 1080";
        let result = parse(input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].verb, Verb::Resize);
        assert_eq!(result[0].selector, Selector::Class("Chrome".to_string()));
        assert_eq!(result[0].args, vec!["1920", "1080"]);
    }

    #[test]
    fn test_parse_fullscreen() {
        let input = "FULLSCREEN title:YouTube";
        let result = parse(input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].verb, Verb::Fullscreen);
        assert_eq!(result[0].selector, Selector::Title("YouTube".to_string()));
    }

    #[test]
    fn test_parse_swap() {
        let input = "SWAP title:Left class:Right";
        let result = parse(input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].verb, Verb::Swap);
        assert_eq!(result[0].selector, Selector::Title("Left".to_string()));
        assert_eq!(result[0].args, vec!["class:Right"]);
    }

    #[test]
    fn test_parse_case_insensitive_verb() {
        let input = "focus title:Test";
        let result = parse(input).unwrap();
        assert_eq!(result[0].verb, Verb::Focus);
    }

    #[test]
    fn test_parse_case_insensitive_direction() {
        let input = "MOVE title:Win right";
        let result = parse(input).unwrap();
        assert_eq!(result[0].args, vec!["right"]);
    }

    #[test]
    fn test_parse_none() {
        let input = "NONE";
        let result = parse(input).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_parse_blank_lines_and_comments() {
        let input = r#"
# This is a comment
FOCUS title:Firefox

# Another comment
CLOSE class:Terminal
"#;
        let result = parse(input).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].verb, Verb::Focus);
        assert_eq!(result[1].verb, Verb::Close);
    }

    #[test]
    fn test_parse_unknown_verb() {
        let input = "INVALID title:Test";
        let result = parse(input);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown verb: INVALID"));
    }

    #[test]
    fn test_parse_missing_selector() {
        let input = "FOCUS";
        let result = parse(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing selector"));
    }

    #[test]
    fn test_parse_invalid_selector_format() {
        let input = "FOCUS InvalidSelector";
        let result = parse(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_move_missing_direction() {
        let input = "MOVE title:Win";
        let result = parse(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("direction"));
    }

    #[test]
    fn test_parse_move_invalid_direction() {
        let input = "MOVE title:Win DIAGONAL";
        let result = parse(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_resize_missing_args() {
        let input = "RESIZE class:Win 1920";
        let result = parse(input);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("two numeric arguments"));
    }

    #[test]
    fn test_parse_resize_non_numeric() {
        let input = "RESIZE class:Win abc 1080";
        let result = parse(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_swap_missing_second_selector() {
        let input = "SWAP title:Win1";
        let result = parse(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("second selector"));
    }

    #[test]
    fn test_selector_with_spaces_in_value() {
        let input = "FOCUS title:My Window Title";
        let result = parse(input).unwrap();
        assert_eq!(result[0].selector, Selector::Title("My".to_string()));
    }

    #[test]
    fn test_to_hyprctl_focus() {
        let cmd = Command {
            verb: Verb::Focus,
            selector: Selector::Title("Firefox".to_string()),
            args: vec![],
        };
        assert_eq!(cmd.to_hyprctl(), "dispatch focuswindow title:Firefox");
    }

    #[test]
    fn test_to_hyprctl_close() {
        let cmd = Command {
            verb: Verb::Close,
            selector: Selector::Class("Alacritty".to_string()),
            args: vec![],
        };
        assert_eq!(cmd.to_hyprctl(), "dispatch closewindow class:Alacritty");
    }

    #[test]
    fn test_to_hyprctl_move() {
        let cmd = Command {
            verb: Verb::Move,
            selector: Selector::Title("Terminal".to_string()),
            args: vec!["LEFT".to_string()],
        };
        assert_eq!(
            cmd.to_hyprctl(),
            "dispatch focuswindow title:Terminal ; dispatch movewindow l"
        );
    }

    #[test]
    fn test_to_hyprctl_resize() {
        let cmd = Command {
            verb: Verb::Resize,
            selector: Selector::Class("Chrome".to_string()),
            args: vec!["50".to_string(), "50".to_string()],
        };
        assert_eq!(
            cmd.to_hyprctl(),
            "dispatch focuswindow class:Chrome ; dispatch resizeactive 50% 50%"
        );
    }

    #[test]
    fn test_to_hyprctl_fullscreen() {
        let cmd = Command {
            verb: Verb::Fullscreen,
            selector: Selector::Title("Video".to_string()),
            args: vec![],
        };
        assert_eq!(
            cmd.to_hyprctl(),
            "dispatch focuswindow title:Video ; dispatch fullscreen 1"
        );
    }

    #[test]
    fn test_to_hyprctl_swap() {
        let cmd = Command {
            verb: Verb::Swap,
            selector: Selector::Title("Left".to_string()),
            args: vec!["class:Right".to_string()],
        };
        assert_eq!(
            cmd.to_hyprctl(),
            "dispatch focuswindow title:Left ; dispatch swapwindow class:Right"
        );
    }
}
