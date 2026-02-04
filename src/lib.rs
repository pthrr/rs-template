#[cfg(test)]
use mockall::automock;

pub fn greet(name: &str) -> String {
    return format_greeting(name);
}

fn format_greeting(name: &str) -> String {
    return format!("Hello, {name}!");
}

#[cfg_attr(test, automock)]
pub trait Greeter {
    fn greet(&self, name: &str) -> String;
}

pub struct FriendlyGreeter;

impl Greeter for FriendlyGreeter {
    fn greet(&self, name: &str) -> String {
        return format!("Hello, {name}!");
    }
}

pub struct FormalGreeter;

impl Greeter for FormalGreeter {
    fn greet(&self, name: &str) -> String {
        return format!("Good day, {name}.");
    }
}

pub trait Named {
    fn name(&self) -> &str;
}

pub trait Displayable: Named {
    fn display(&self) -> String {
        return format!("[{}]", self.name());
    }
}

pub trait Interactive: Named + Greeter {
    fn interact(&self, target: &str) -> String {
        let intro = format!("I am {}. ", self.name());
        let greeting = self.greet(target);
        return intro + &greeting;
    }
}

pub struct GreeterBot {
    name: String,
}

impl GreeterBot {
    pub fn new(name: &str) -> Self {
        return Self {
            name: name.to_string(),
        };
    }

    pub fn process_greeting(&self, target: &str) -> String {
        return self.interact(target);
    }
}

impl Named for GreeterBot {
    fn name(&self) -> &str {
        return &self.name;
    }
}

impl Greeter for GreeterBot {
    fn greet(&self, name: &str) -> String {
        return format!("Greetings, {name}!");
    }
}

impl Interactive for GreeterBot {}

impl Displayable for GreeterBot {}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_greet() {
        assert_eq!(greet("Rust"), "Hello, Rust!");
    }

    #[test]
    fn test_friendly_greeter() {
        let greeter = FriendlyGreeter;
        assert_eq!(greeter.greet("World"), "Hello, World!");
    }

    #[test]
    fn test_formal_greeter() {
        let greeter = FormalGreeter;
        assert_eq!(greeter.greet("World"), "Good day, World.");
    }

    #[test]
    fn test_greeter_bot() {
        let bot = GreeterBot::new("R2D2");
        assert_eq!(bot.name(), "R2D2");
        assert!(bot.greet("Alice").contains("Greetings"));
        assert!(bot.interact("Bob").contains("I am R2D2"));
    }

    // Mockall tests
    #[test]
    fn test_mock_greeter() {
        let mut mock = MockGreeter::new();
        let _ = mock
            .expect_greet()
            .with(mockall::predicate::eq("Alice"))
            .times(1)
            .returning(|name| format!("Mocked greeting for {name}!"));

        assert_eq!(mock.greet("Alice"), "Mocked greeting for Alice!");
    }

    #[test]
    fn test_mock_greeter_any_input() {
        let mut mock = MockGreeter::new();
        let _ = mock.expect_greet().returning(|name| format!("Hi, {name}!"));

        assert_eq!(mock.greet("Bob"), "Hi, Bob!");
        assert_eq!(mock.greet("Charlie"), "Hi, Charlie!");
    }

    // Proptest tests
    proptest! {
        #[test]
        fn test_greet_contains_name(name in "[a-zA-Z]{1,20}") {
            let result = greet(&name);
            prop_assert!(result.contains(&name));
            prop_assert!(result.starts_with("Hello, "));
            prop_assert!(result.ends_with('!'));
        }

        #[test]
        fn test_friendly_greeter_format(name in "[a-zA-Z]{1,20}") {
            let greeter = FriendlyGreeter;
            let result = greeter.greet(&name);
            prop_assert_eq!(result, format!("Hello, {name}!"));
        }

        #[test]
        fn test_formal_greeter_format(name in "[a-zA-Z]{1,20}") {
            let greeter = FormalGreeter;
            let result = greeter.greet(&name);
            prop_assert_eq!(result, format!("Good day, {name}."));
        }

        #[test]
        fn test_greeter_bot_interact_contains_both_names(
            bot_name in "[a-zA-Z]{1,10}",
            target in "[a-zA-Z]{1,10}"
        ) {
            let bot = GreeterBot::new(&bot_name);
            let result = bot.interact(&target);
            prop_assert!(result.contains(&bot_name));
            prop_assert!(result.contains(&target));
        }
    }
}
