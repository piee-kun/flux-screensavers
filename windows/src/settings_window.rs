use glow::*;
use glutin::dpi::PhysicalPosition;
use glutin::event::{Event, ModifiersState, WindowEvent};
use iced::widget::{column, container, pick_list, scrollable, vertical_space};
use iced::{Alignment, Element, Length, Sandbox};
use iced_glow::glow;
use iced_glutin::conversion;
use iced_glutin::glutin;
use iced_glutin::renderer;
use iced_glutin::{program, Clipboard, Debug, Size};

pub fn run() -> iced::Result {
    Settings::run(iced::Settings::default())
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

        let content = column![
            vertical_space(Length::Units(600)),
            "Color theme",
            pick_list,
            vertical_space(Length::Units(600)),
        ]
        .width(Length::Fill)
        .align_items(Alignment::Center)
        .spacing(10);

        container(scrollable(content))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Color {
    Plasma,
    Poolside,
    Desktop,
}

impl Color {
    const ALL: [Color; 3] = [Color::Plasma, Color::Poolside, Color::Desktop];
}

impl Default for Color {
    fn default() -> Color {
        Color::Plasma
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Color::Plasma => "Plasma",
                Color::Poolside => "Poolside",
                Color::Desktop => "Use desktop wallpaper",
            }
        )
    }
}
