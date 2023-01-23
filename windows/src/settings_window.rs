use crate::config::Config;

use iced::widget::{column, container, pick_list};
use iced::{Alignment, Element, Length, Sandbox};

pub fn run(config: Config) -> iced::Result {
    Settings::run(iced::Settings {
        window: iced::window::Settings {
            size: (250, 250),
            resizable: false,
            decorations: true,
            ..Default::default()
        },
        ..Default::default()
    })
}

#[derive(Default)]
struct Settings {
    color: Color,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    SetColor(Color),
}

impl Sandbox for Settings {
    type Message = Message;

    fn new() -> Self {
        Self::default()
    }

    fn title(&self) -> String {
        String::from("Flux Settings")
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::SetColor(new_color) => {
                self.color = new_color;
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let pick_list = pick_list(&Color::ALL[..], Some(self.color), Message::SetColor)
            .placeholder("Choose a color theme");

        let content = column!["Colors", pick_list,]
            .width(Length::Fill)
            .align_items(Alignment::Center)
            .spacing(10)
            .padding(10);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }
}

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq)]
enum Color {
    #[default]
    Original,
    Plasma,
    Poolside,
    Desktop,
}

impl Color {
    const ALL: [Color; 4] = [
        Color::Original,
        Color::Plasma,
        Color::Poolside,
        Color::Desktop,
    ];
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Color::Original => "Original",
                Color::Plasma => "Plasma",
                Color::Poolside => "Poolside",
                Color::Desktop => "Use desktop wallpaper",
            }
        )
    }
}
