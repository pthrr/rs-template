pub fn greet(name: &str) -> String {
    format_greeting(name)
}

fn format_greeting(name: &str) -> String {
    format!("Hello, {}!", name)
}

pub trait Greeter {
    fn greet(&self, name: &str) -> String;
}

pub struct FriendlyGreeter;

impl Greeter for FriendlyGreeter {
    fn greet(&self, name: &str) -> String {
        format!("Hello, {}!", name)
    }
}

pub struct FormalGreeter;

impl Greeter for FormalGreeter {
    fn greet(&self, name: &str) -> String {
        format!("Good day, {}.", name)
    }
}

pub trait Named {
    fn name(&self) -> &str;
}

pub trait Displayable: Named {
    fn display(&self) -> String {
        format!("[{}]", self.name())
    }
}

pub trait Interactive: Named + Greeter {
    fn interact(&self, target: &str) -> String {
        let intro = format!("I am {}. ", self.name());
        let greeting = self.greet(target);
        intro + &greeting
    }
}

pub struct GreeterBot {
    name: String,
}

impl GreeterBot {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }

    pub fn process_greeting(&self, target: &str) -> String {
        self.interact(target)
    }
}

impl Named for GreeterBot {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Greeter for GreeterBot {
    fn greet(&self, name: &str) -> String {
        format!("Greetings, {}!", name)
    }
}

impl Interactive for GreeterBot {}

impl Displayable for GreeterBot {}

#[cfg(test)]
mod tests {
    use super::*;

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
}
