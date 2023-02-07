use crate::config::{ColorMode, Config};

use iced::executor;
use iced::widget::{button, column, container, pick_list, text};
use iced::{Alignment, Application, Command, Element, Length, Theme};

pub fn run(config: Config) -> iced::Result {
    Config::run(iced::Settings {
        flags: config,
        window: iced::window::Settings {
            size: (250, 250),
            resizable: false,
            decorations: true,
            ..Default::default()
        },
        ..Default::default()
    })
}

#[derive(Debug, Clone, Copy)]
pub enum Message {
    SetColorMode(ColorMode),
    Save,
}

impl Application for Config {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = Config;

    fn new(config: Config) -> (Self, Command<Message>) {
        (config, Command::none())
    }

    fn title(&self) -> String {
        String::from("Flux Settings")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::SetColorMode(new_color) => {
                self.flux.color_mode = new_color;
                Command::none()
            }

            Message::Save => {
                self.save().unwrap_or_else(|err| log::error!("{}", err));
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let pick_list = pick_list(
            &ColorMode::ALL[..],
            Some(self.flux.color_mode),
            Message::SetColorMode,
        )
        .placeholder("Choose a color theme");

        let save_button = button(text("Save")).on_press(Message::Save);

        let content = column!["Colors", pick_list, save_button]
            .height(Length::Fill)
            .align_items(Alignment::Center)
            .spacing(10);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .padding(10)
            .into()
    }
}
